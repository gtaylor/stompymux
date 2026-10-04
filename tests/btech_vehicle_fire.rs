//! Complete vehicle shots reuse shared aim, launch, defenses and target damage with atomic replay.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        BattleMapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: BattlePower) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    for id in ids {
        let class = if world.btech.vehicles().contains_key(id) {
            "vehicles"
        } else {
            "constructed"
        };
        saved[class][id.0.to_string()]["power"] = serde_json::to_value(value).unwrap();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

/// Ordinary conventional aim without range extensions or arc overrides.
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

/// The shooter's turret points toward both target classes at a one-hex distance.
async fn engagement(template: &str) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(".0\n.0\n.0\n.0\n.0\n", template).await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    place_battle_unit(&mut world, ids[2], map, 0, 1).unwrap();
    power(&mut world, &ids, BattlePower::Running);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[2]);
    assign_battle_pilot(&mut world, ids[2], ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[ids[2]]).unwrap();
    (dir, config, world, map, ids)
}

/// Add a one-slot defense and matching bin without replacing the fixture's offensive mounts.
fn install_test_ams(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    weapon: stompymux_rs::BattleWeapon,
) -> (usize, usize) {
    install_test_ams_at(world, id, weapon, stompymux_rs::BattleSection::LeftArm)
}

/// Supply one arm-mounted defense for equipment selection scenarios.
fn install_test_ams_at(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    weapon: stompymux_rs::BattleWeapon,
    section: stompymux_rs::BattleSection,
) -> (usize, usize) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let arm = definition.sections.get_mut(&section).unwrap();
    let mut part = arm.criticals[&2].clone();
    part.equipment = weapon.name().into();
    arm.criticals.insert(4, part.clone());
    part.equipment = format!("Ammo_{}", weapon.name());
    part.data = weapon.profile().ammunition_per_ton.to_string();
    arm.criticals.insert(5, part);
    let constructed = BattleUnit::from_template(definition.clone()).unwrap();
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

/// Explicit shooter and victim critical policies for tactical firing tests.
fn fire_rules() -> BattleVehicleShotRules {
    let criticals = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Standard,
        enabled: false,
        combat_safe: false,
        toughness: false,
    };
    BattleVehicleShotRules {
        shot: BattleShotRules {
            range_damage: false,
            tsm_tow_bonus: true,
            vehicle_impact: BattleVehicleImpactRules {
                advanced_fire: false,
                criticals,
                hit: BattleVehicleHitRules {
                    critical_mode: 1,
                    critical_level: 40,
                },
            },
            ..shot_rules()
        },
        shooter_criticals: criticals,
    }
}

/// Seed only shooter attack dice; target dice and material state remain independently owned.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        })
        .unwrap();
}

#[tokio::test]
async fn vehicle_shots_commit_hits_misses_and_restart_replay_for_both_target_classes() {
    let (_dir, config, base, _map, [mech, _, shooter, vehicle]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    for target in [mech, vehicle] {
        for hit in [false, true] {
            let mut world = base.clone();
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
            let preview =
                check_battle_vehicle_shot(&world, shooter, ObjectId(1), target, 0, shot_rules())
                    .unwrap();
            let before = world.btech.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(
                report,
                fire_battle_vehicle_shot(
                    &mut replay,
                    shooter,
                    ObjectId(1),
                    target,
                    0,
                    fire_rules()
                )
                .unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(report.aim, preview);
            assert_eq!(report.launch.hit, hit);
            assert!(report.launch.expenditure.launched);
            assert_eq!(report.salvo.is_some(), hit);
            assert_eq!(
                report
                    .launch
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                1
            );
            let attacker = &world.btech.vehicles()[&shooter];
            let readiness = attacker.weapon_readiness(0).unwrap();
            if attacker.is_destroyed() || !readiness.intact {
                assert!(!readiness.ready);
                let Some(BattleTargetSalvo::Mech(salvo)) = &report.salvo else {
                    panic!("The return blast must come from the Mech target");
                };
                assert!(
                    salvo
                        .groups
                        .iter()
                        .flat_map(|group| &group.impact.reactor_explosions)
                        .any(|blast| blast.hits.iter().any(|hit| hit.unit == shooter))
                );
            } else {
                assert!(readiness.recycle_remaining > 0);
            }
            if target == mech {
                assert_eq!(
                    world.btech.constructed_units()[&target] == before.constructed_units()[&target],
                    !hit
                );
                if hit {
                    assert!(matches!(report.salvo, Some(BattleTargetSalvo::Mech(_))));
                }
            } else {
                assert_eq!(
                    world.btech.vehicles()[&target] == before.vehicles()[&target],
                    !hit
                );
                if hit {
                    assert!(matches!(report.salvo, Some(BattleTargetSalvo::Vehicle(_))));
                }
            }
            let after = world.btech.clone();
            assert!(
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .is_err()
            );
            assert_eq!(world.btech, after);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_missiles_use_shooter_dice_for_mech_ams_only_on_admitted_hits() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.SRM-6");
    let (_dir, config, mut base, _map, [target, _, shooter, _]) = engagement(&template).await;
    install_test_ams(&mut base, target, BattleWeapon::AntiMissileSystem);
    base.btech
        .rewrite_unit_record(target, |record| {
            record["ams_enabled"] = serde_json::json!(true);
        })
        .unwrap();
    for hit in [false, true] {
        let mut world = base.clone();
        let value = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
            .unwrap();
        seed(&mut world, shooter, value);
        let mut dice = BattleDice::seeded([value; 32]);
        let attack = dice.two_d6();
        let before = world.btech.constructed_units()[&target]
            .ammunition()
            .to_vec();
        let report =
            fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                .unwrap();
        assert_eq!(report.launch.roll, attack);
        assert_eq!(report.launch.hit, hit);
        if !hit {
            assert!(report.ams.is_none());
            assert_eq!(
                world.btech.constructed_units()[&target].ammunition(),
                before
            );
            assert_eq!(roll_unit_dice(&mut world, shooter, 1).unwrap(), [dice.d6()]);
            world.validate(&config).unwrap();
            continue;
        }
        let defense = dice.d6();
        let ams = report.ams.unwrap();
        assert_eq!(ams.roll, defense);
        assert_eq!(ams.ammunition_spent, u16::from(defense));
        assert_eq!(
            world.btech.constructed_units()[&target].ammunition()[ams.ammunition_bin.unwrap()],
            before[ams.ammunition_bin.unwrap()] - u16::from(defense)
        );
        assert_eq!(roll_unit_dice(&mut world, shooter, 1).unwrap(), [dice.d6()]);
        world.validate(&config).unwrap();
    }
}

/// A failed Streak lock spends nothing, and bad pilots, unperceived or in-character targets reject shots.
#[tokio::test]
async fn vehicle_shot_failures_and_failed_streak_locks_preserve_target_state() {
    let (_dir, _config, mut base, _map, [_, _, shooter, target]) =
        engagement(include_str!("../game/mechs/Svantovit-Streak.toml")).await;
    let index = base.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.is_streak())
        .unwrap();
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 2)
        .unwrap();
    seed(&mut base, shooter, value);
    let before_target = base.btech.vehicles()[&target].clone();
    let before_ammo = base.btech.vehicles()[&shooter].ammunition().to_vec();
    let mut rules = fire_rules();
    rules.shot.aim.override_weapon_arcs = true;
    let report =
        fire_battle_vehicle_shot(&mut base, shooter, ObjectId(1), target, index, rules).unwrap();
    assert!(!report.launch.expenditure.launched);
    assert!(report.salvo.is_none());
    assert_eq!(base.btech.vehicles()[&shooter].ammunition(), before_ammo);
    assert_eq!(base.btech.vehicles()[&target], before_target);
    let (_dir, _config, base, map, [_, _, shooter, target]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    for case in ["pilot", "contact", "character"] {
        let mut world = base.clone();
        let pilot = if case == "pilot" {
            ObjectId(999)
        } else {
            ObjectId(1)
        };
        if case == "contact" {
            // Without the sensor band or any sight the target is no longer perceived.
            set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false)
                .unwrap();
            set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
        }
        if case == "character" {
            world
                .objects
                .get_mut(&target)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
        }
        let before = world.btech.clone();
        assert!(
            fire_battle_vehicle_shot(&mut world, shooter, pilot, target, 0, fire_rules()).is_err()
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn vehicle_shots_observe_existing_angel_fields_without_copying_field_rules() {
    for weapon in [
        "IS.StreakSRM-6",
        "CL.StreakLRM-5",
        "CL.StreakLRM-10",
        "CL.StreakLRM-15",
        "CL.StreakLRM-20",
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", weapon);
        let (_dir, config, mut base, _map, [_, emitter, shooter, target]) =
            engagement(&template).await;
        let mut definition = base.btech.constructed_units()[&emitter]
            .definition()
            .clone();
        for slot in 2..4 {
            definition
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "AngelEcm".into(),
                        data: "-".into(),
                        modes: vec![],
                    },
                );
        }
        base.btech
            .rewrite_unit_record(emitter, |record| {
                record["definition"] = serde_json::to_value(definition).unwrap();
                record["signature"]["team"] = serde_json::json!(1);
            })
            .unwrap();
        let value = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 2)
            .unwrap();
        seed(&mut base, shooter, value);
        for active in [false, true] {
            let mut world = base.clone();
            world
                .btech
                .rewrite_unit_record(emitter, |record| {
                    record["electronics"]["angel"] = serde_json::to_value(if active {
                        BattleElectronicMode::Ecm
                    } else {
                        BattleElectronicMode::Off
                    })
                    .unwrap();
                })
                .unwrap();
            let before = world.btech.clone();
            assert_eq!(
                battle_electronic_field(&world, shooter)
                    .unwrap()
                    .angel_disturbed,
                active
            );
            assert_eq!(world.btech, before);
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(report.launch.expenditure.launched, active);
            assert!(!report.launch.hit);
            assert!(report.salvo.is_none());
            assert_eq!(
                report
                    .launch
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                u16::from(active)
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_native_and_lua_fire_share_state_feedback_and_callback_rollback() {
    let (_dir, config, base, _map, [mech, _, shooter, vehicle]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    for target in [mech, vehicle] {
        for hit in [false, true] {
            let mut world = base.clone();
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
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
            let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(
                lua.drain_outbox().is_empty(),
                "Aborted firing leaked feedback"
            );
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire 0 #{}", target.0),
            );
            assert!(output.contains("You fire IS.AC/20"), "{output}");
            assert!(
                output.contains(if hit { "Hit." } else { "Miss." }),
                "{output}"
            );
            let actual = lua
                .eval_callback::<bool>(&format!("return {call}.launch.hit"))
                .unwrap();
            assert_eq!(actual, hit);
            assert_eq!(lua.world().btech, native.world().btech);
            lua.world().validate(&config).unwrap();
            let before = native.world().btech.clone();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                2,
                &format!("fire 0 #{}", target.0),
            );
            assert!(!output.contains("You fire"), "{output}");
            assert_eq!(native.world().btech, before);
        }
    }
}

#[tokio::test]
async fn vehicle_occupied_hex_fire_preserves_selection_and_native_lua_parity() {
    let (_dir, config, base, map, [mech, other, shooter, vehicle]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    let hex = BattleHexCoordinate { x: 0, y: 0 };
    for target in [mech, vehicle] {
        let mut world = base.clone();
        power(&mut world, &[mech, other], BattlePower::Off);
        place_battle_unit(&mut world, other, map, 0, 4).unwrap();
        if target == vehicle {
            place_battle_unit(&mut world, mech, map, 0, 4).unwrap();
        }
        power(&mut world, &[mech, other], BattlePower::Running);
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        assert_eq!(
            battle_hex_occupant(&world, shooter, hex).unwrap(),
            Some(target)
        );
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            hex,
            BattleHexTargetMode::UnitAtHex,
        )
        .unwrap();
        // This case checks coordinate selection and presentation; damage is covered separately.
        let value = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 2)
            .unwrap();
        seed(&mut world, shooter, value);
        let selection = world.btech.vehicles()[&shooter].target_selection();
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
        let output = support::run_text(&native, &config, ObjectId(1), 1, "fire 0");
        assert!(output.contains("You fire AC/20 at (0,0)"), "{output}");
        let report = lua
            .eval_callback::<(i64, i32, i32)>(&format!(
                "local r=btech.unit.fire({},1,0); return r.target,r.coordinate.x,r.coordinate.y",
                shooter.0
            ))
            .unwrap();
        assert_eq!(report, (target.0, 0, 0));
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            lua.world().btech.vehicles()[&shooter].target_selection(),
            selection
        );
        lua.world().validate(&config).unwrap();
        let explicit =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let output = support::run_text(
            &explicit,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert!(
            output.contains(&format!("You fire IS.AC/20 at #{}", target.0)),
            "{output}"
        );
        assert_eq!(
            explicit.world().btech.vehicles()[&shooter].target_selection(),
            selection
        );
    }
}

#[tokio::test]
async fn occupied_hex_selection_does_not_skip_hidden_or_forbidden_targets() {
    let (_dir, config, mut base, map, [mech, other, shooter, vehicle]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    let hex = BattleHexCoordinate { x: 0, y: 0 };
    select_battle_hex_target(
        &mut base,
        shooter,
        ObjectId(1),
        hex,
        BattleHexTargetMode::UnitAtHex,
    )
    .unwrap();
    for condition in ["hidden", "character", "empty", "terrain"] {
        let mut world = base.clone();
        match condition {
            "hidden" => {
                world
                    .btech
                    .rewrite_unit_record(shooter, |record| {
                        record["contacts"]
                            .as_object_mut()
                            .unwrap()
                            .remove(&mech.0.to_string());
                    })
                    .unwrap();
            }
            "character" => {
                world
                    .objects
                    .get_mut(&mech)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            "empty" => {
                power(&mut world, &[mech, other, vehicle], BattlePower::Off);
                for id in [mech, other, vehicle] {
                    place_battle_unit(&mut world, id, map, 0, 4).unwrap();
                }
            }
            "terrain" => {
                select_battle_hex_target(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    hex,
                    BattleHexTargetMode::Hex,
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let before = world.btech.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        if matches!(condition, "empty" | "terrain") {
            let report: (bool, bool) = scripts
                .eval_callback(&format!(
                    "local r=btech.unit.fire({},1,0); return r.target==nil,r.launched",
                    shooter.0
                ))
                .unwrap();
            assert_eq!(report, (true, true), "{condition}");
            let after = scripts.world();
            for id in [mech, other, vehicle] {
                assert_eq!(
                    before.constructed_units().get(&id),
                    after.btech.constructed_units().get(&id)
                );
                assert_eq!(before.vehicles().get(&id), after.btech.vehicles().get(&id));
            }
            assert_eq!(
                before.vehicles()[&shooter].ammunition().iter().sum::<u16>()
                    - after.btech.vehicles()[&shooter]
                        .ammunition()
                        .iter()
                        .sum::<u16>(),
                1
            );
            continue;
        }
        if condition == "character" {
            let selected: i64 = scripts
                .eval_callback(&format!("return btech.unit.fire({},1,0).target", shooter.0))
                .unwrap();
            assert_eq!(selected, mech.0);
            continue;
        }
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.fire({},1,0)", shooter.0))
                .is_err(),
            "{condition}"
        );
        assert_eq!(scripts.world().btech, before, "{condition}");
        assert!(scripts.drain_outbox().is_empty());
    }
    base.objects
        .get_mut(&mech)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert_eq!(
        battle_hex_occupant(&base, shooter, hex).unwrap(),
        Some(other)
    );
    assert!(battle_hex_occupant(&base, shooter, BattleHexCoordinate { x: 9, y: 9 }).is_err());
}

/// Replace the target's turret mount and its bin while preserving battlefield membership. Laser
/// AMS draws no ammunition, so it takes no bin.
fn install_vehicle_ams(world: &mut World, target: ObjectId, weapon: BattleWeapon) {
    let template = include_str!("../game/mechs/Demolisher.toml");
    let source = if weapon.profile().ammunition_per_ton > 0 {
        template.replace("IS.AC/20", weapon.name())
    } else {
        template
            .replace(
                "    { at = \"3-6\", item = \"Ammo_IS.AC/20\", rounds = 5 },\n",
                "",
            )
            .replace("IS.AC/20", weapon.name())
    };
    let unit = BattleVehicle::new(BattleVehicleTemplate::parse("test", &source).unwrap()).unwrap();
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["definition"] = serde_json::to_value(unit.definition()).unwrap();
            record["ammunition"] = serde_json::to_value(unit.ammunition()).unwrap();
            record["ams_enabled"] = serde_json::json!(true);
        })
        .unwrap();
}

#[tokio::test]
async fn vehicle_ams_shares_interception_dice_supply_limits_and_restart_replay() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.SRM-6");
    let (_dir, config, base, _, [_, _, shooter, target]) = engagement(&template).await;
    for weapon in [
        BattleWeapon::AntiMissileSystem,
        BattleWeapon::ClanAntiMissileSystem,
        BattleWeapon::LaserAms,
        BattleWeapon::ClanLaserAms,
    ] {
        for hit in [false, true] {
            let mut world = base.clone();
            install_vehicle_ams(&mut world, target, weapon);
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
            let mut dice = BattleDice::seeded([value; 32]);
            dice.two_d6();
            let fed = weapon.profile().ammunition_per_ton > 0;
            let rounds = world.btech.vehicles()[&target]
                .ammunition()
                .first()
                .copied()
                .unwrap_or(0);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert!(replay.btech.vehicles()[&target].ams_enabled());
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(
                report,
                fire_battle_vehicle_shot(
                    &mut replay,
                    shooter,
                    ObjectId(1),
                    target,
                    0,
                    fire_rules()
                )
                .unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            if !hit {
                assert!(report.ams.is_none());
                assert!(report.salvo.is_none());
                if fed {
                    assert_eq!(world.btech.vehicles()[&target].ammunition()[0], rounds);
                }
                assert_eq!(world.btech.vehicles()[&target].weapon_heat(), 0.0);
                assert!(world.btech.vehicles()[&target].weapon_recycle().is_empty());
                assert_eq!(roll_unit_dice(&mut world, shooter, 1).unwrap(), [dice.d6()]);
                world.validate(&config).unwrap();
                continue;
            }
            let defense = if matches!(
                weapon,
                BattleWeapon::ClanAntiMissileSystem | BattleWeapon::ClanLaserAms
            ) {
                dice.two_d6()
            } else {
                dice.d6()
            };
            let ams = report.ams.as_ref().unwrap();
            assert_eq!(ams.roll, defense);
            assert_eq!(
                world.btech.vehicles()[&target].weapon_heat(),
                f64::from(weapon.profile().heat)
            );
            assert_eq!(
                (ams.weapon_index, ams.ammunition_bin),
                (0, fed.then_some(0))
            );
            if fed {
                assert_eq!(ams.ammunition_spent, rounds.min(u16::from(defense.min(6))));
                assert_eq!(
                    world.btech.vehicles()[&target].ammunition()[0],
                    rounds - ams.ammunition_spent
                );
            } else {
                assert_eq!(ams.ammunition_spent, 0);
            }
            assert_eq!(
                world.btech.vehicles()[&target].weapon_recycle()[&0],
                u16::from(weapon.profile().recycle_seconds)
            );
            let Some(BattleTargetSalvo::Vehicle(salvo)) = &report.salvo else {
                panic!("vehicle salvo missing")
            };
            let missiles = salvo.missiles_before_defense.unwrap();
            assert_eq!(ams.shot_down, defense.min(missiles));
            assert_eq!(
                salvo.groups.iter().map(|group| group.damage).sum::<u16>(),
                u16::from(missiles - ams.shot_down) * 2
            );
            assert_eq!(roll_unit_dice(&mut world, shooter, 1).unwrap(), [dice.d6()]);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_ams_switch_shares_native_lua_control_and_callback_rollback() {
    let (_dir, config, mut base, _, [_, _, shooter, target]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    assert!(!base.btech.vehicles()[&target].ams_enabled());
    assert!(set_battle_ams(&mut base, shooter, ObjectId(1), true).is_err());
    install_vehicle_ams(&mut base, target, BattleWeapon::AntiMissileSystem);
    release_battle_pilot(&mut base, shooter, ObjectId(1)).unwrap();
    base.objects.get_mut(&ObjectId(1)).unwrap().location = Some(target);
    assign_battle_pilot(&mut base, target, ObjectId(1)).unwrap();
    assert!(set_battle_ams(&mut base, target, ObjectId(2), true).is_err());
    set_battle_ams(&mut base, target, ObjectId(1), false).unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.ams({},1,true); error('abort')",
                target.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, base.btech);
    assert!(scripts.drain_outbox().is_empty());
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "ams").contains("turned ON"));
    assert!(
        scripts
            .eval_callback::<bool>(&format!(
                "return btech.unit.state({}).ams_enabled",
                target.0
            ))
            .unwrap()
    );
    assert!(
        !scripts
            .eval_callback::<bool>(&format!("return btech.unit.ams({},1)", target.0))
            .unwrap()
    );
    assert!(!scripts.world().btech.vehicles()[&target].ams_enabled());
    scripts.world().validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_ams_selection_obeys_switch_supply_recycle_and_critical_loss() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.SRM-6");
    let (_dir, config, mut base, _, [_, _, shooter, target]) = engagement(&template).await;
    install_vehicle_ams(&mut base, target, BattleWeapon::AntiMissileSystem);
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut base, shooter, value);
    for condition in [
        "disabled",
        "off",
        "recycling",
        "empty",
        "critical",
        "first_busy",
        "first_failed",
        "first_bin_lost",
    ] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(target, |record| {
                let unit = record;
                match condition {
                    "disabled" => unit["ams_enabled"] = serde_json::json!(false),
                    "off" => unit["power"] = serde_json::to_value(BattlePower::Off).unwrap(),
                    "recycling" => unit["weapon_recycle"] = serde_json::json!({"0":1,"1":1}),
                    "empty" => unit["ammunition"] = serde_json::json!([0, 0, 0, 0]),
                    "first_busy" => unit["weapon_recycle"] = serde_json::json!({"0":1}),
                    "first_failed" => unit["weapon_failures"] = serde_json::json!({"0":"disabled"}),
                    _ => {}
                }
            })
            .unwrap();
        if matches!(condition, "critical" | "first_bin_lost") {
            let location = if condition == "critical" {
                world.btech.vehicles()[&target].loadout().unwrap().weapons[0].criticals[0]
            } else {
                world.btech.vehicles()[&target]
                    .loadout()
                    .unwrap()
                    .ammunition[0]
                    .location
            };
            destroy_battle_vehicle_critical(&mut world, target, location).unwrap();
        }
        let mut dice = BattleDice::seeded([value; 32]);
        dice.two_d6();
        let report =
            fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                .unwrap();
        if matches!(condition, "first_busy" | "first_failed" | "first_bin_lost") {
            assert_eq!(report.ams.as_ref().unwrap().roll, dice.d6());
            assert_eq!(
                report.ams.as_ref().unwrap().weapon_index,
                usize::from(condition != "first_bin_lost")
            );
            assert_eq!(
                report.ams.as_ref().unwrap().ammunition_bin,
                Some(usize::from(condition == "first_bin_lost"))
            );
        } else {
            assert!(report.ams.is_none(), "{condition}");
        }
        assert_eq!(
            roll_unit_dice(&mut world, shooter, 1).unwrap(),
            [dice.d6()],
            "{condition}"
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_ams_expenditure_and_feedback_roll_back_with_host_shots() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.SRM-6");
    let (_dir, config, mut base, _, [_, _, shooter, target]) = engagement(&template).await;
    install_vehicle_ams(&mut base, target, BattleWeapon::ClanAntiMissileSystem);
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut base, shooter, value);
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
    let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    assert!(lua.drain_outbox().is_empty());
    let output = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    assert!(output.contains("You fire"), "{output}");
    assert!(
        lua.eval_callback::<bool>(&format!("return {call}.ams ~= nil"))
            .unwrap()
    );
    assert_eq!(lua.world().btech, native.world().btech);
    lua.world().validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_coolant_and_flamer_heat_share_target_effects_and_host_rollback() {
    for weapon in [
        BattleWeapon::CoolantGun,
        BattleWeapon::VehicleFlamer,
        BattleWeapon::VehicleHeavyFlamer,
    ] {
        let template =
            include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", weapon.name());
        let (_dir, config, base, _, [target, _, shooter, vehicle]) = engagement(&template).await;
        for hit in [false, true] {
            let mut world = base.clone();
            if weapon != BattleWeapon::CoolantGun {
                toggle_battle_flamer_heat(&mut world, shooter, ObjectId(1), 0).unwrap();
            }
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
            let original = world.btech.clone();
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(report.launch.hit, hit);
            assert!(report.salvo.is_none() && report.narc.is_none());
            let amount = f64::from(weapon.profile().damage);
            let expected = if !hit {
                0.0
            } else if weapon == BattleWeapon::CoolantGun {
                -amount
            } else {
                amount
            };
            assert_eq!(
                world.btech.constructed_units()[&target].heat().stored,
                original.constructed_units()[&target].heat().stored + expected
            );
            assert_eq!(
                world.btech.constructed_units()[&target].sections(),
                original.constructed_units()[&target].sections()
            );
            assert_eq!(
                report.cooling,
                (hit && weapon == BattleWeapon::CoolantGun).then_some(amount)
            );
            assert_eq!(
                report.heat_transfer,
                if hit && weapon != BattleWeapon::CoolantGun {
                    weapon.profile().damage
                } else {
                    0
                }
            );
            let mut initial = base.clone();
            initial.btech = original;
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(initial.clone())),
            )
            .unwrap();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(initial.clone())),
            )
            .unwrap();
            let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, initial.btech);
            assert!(lua.drain_outbox().is_empty());
            lua.eval_callback::<()>(&call).unwrap();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire 0 #{}", target.0),
            );
            assert!(output.contains("You fire"), "{output}");
            assert_eq!(lua.world().btech, native.world().btech);
            assert_eq!(lua.world().btech, world.btech);
            let vehicle_before = initial.btech.vehicles()[&vehicle].clone();
            let vehicle_report = fire_battle_vehicle_shot(
                &mut initial,
                shooter,
                ObjectId(1),
                vehicle,
                0,
                fire_rules(),
            )
            .unwrap();
            assert_eq!(vehicle_report.launch.hit, hit);
            assert_eq!(
                initial.btech.vehicles()[&vehicle].weapon_heat(),
                vehicle_before.weapon_heat() + expected
            );
            assert_eq!(
                initial.btech.vehicles()[&vehicle].sections(),
                vehicle_before.sections()
            );
            assert_eq!(
                world.btech.vehicles()[&shooter].weapon_heat(),
                f64::from(report.launch.expenditure.heat)
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_beacon_launchers_reuse_attachment_interception_and_replay() {
    for (weapon, mode, flag, kind) in [
        (
            BattleWeapon::NarcBeacon,
            BattleAmmunitionMode::Normal,
            "-",
            BattleBeaconKind::Narc,
        ),
        (
            BattleWeapon::ClanNarcBeacon,
            BattleAmmunitionMode::Normal,
            "-",
            BattleBeaconKind::Narc,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::Normal,
            "-",
            BattleBeaconKind::Homing,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::INarcHaywire,
            "iNarc_Haywire",
            BattleBeaconKind::Haywire,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::INarcEcm,
            "iNarc_ECM",
            BattleBeaconKind::Ecm,
        ),
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml")
            .replace("IS.AC/20", weapon.name())
            .replace(
                "rounds = 5 }",
                &if flag == "-" {
                    "rounds = 4 }".to_owned()
                } else {
                    format!("rounds = 4, modes = [\"{flag}\"] }}")
                },
            );
        let (_dir, config, mut base, _, [target, _, shooter, _]) = engagement(&template).await;
        if weapon == BattleWeapon::INarcBeacon {
            set_battle_inarc_ammunition(&mut base, shooter, ObjectId(1), 0, mode).unwrap();
        }
        for (hit, defense) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut world = base.clone();
            if defense {
                install_test_ams(&mut world, target, BattleWeapon::AntiMissileSystem);
                world
                    .btech
                    .rewrite_unit_record(target, |record| {
                        record["ams_enabled"] = serde_json::json!(true);
                    })
                    .unwrap();
            }
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
            let target_before = world.btech.constructed_units()[&target].clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(
                report,
                fire_battle_vehicle_shot(
                    &mut replay,
                    shooter,
                    ObjectId(1),
                    target,
                    0,
                    fire_rules()
                )
                .unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            let pod = report.narc.as_ref().unwrap();
            assert_eq!(
                (pod.kind, pod.hit, pod.intercepted),
                (kind, hit, hit && defense)
            );
            assert_eq!(
                world.btech.constructed_units()[&target].has_beacon(kind),
                hit && !defense
            );
            assert_eq!(
                world.btech.constructed_units()[&target].sections(),
                target_before.sections()
            );
            assert!(report.salvo.is_none());
            assert_eq!(report.ams.is_some(), defense && hit);
            if defense && hit {
                assert_eq!(report.ams.as_ref().unwrap().shot_down, 1);
            }
            if hit && defense {
                assert_eq!(
                    report
                        .notices()
                        .iter()
                        .filter(|notice| notice.text.contains("incoming pod"))
                        .count(),
                    1
                );
            }
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn clan_plasma_vehicle_shots_use_ordinary_damage_packets() {
    // Clan plasma is conventional damage in the shared catalogue, without IS plasma heating.
    let template =
        include_str!("../game/mechs/Demolisher.toml").replace("\"IS.AC/20\"", "\"CL.PlasmaRifle\"");
    let (_dir, config, mut world, _, [mech, _, shooter, vehicle]) = engagement(&template).await;
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut world, shooter, value);
    for target in [mech, vehicle] {
        let mut candidate = world.clone();
        let report = fire_battle_vehicle_shot(
            &mut candidate,
            shooter,
            ObjectId(1),
            target,
            0,
            fire_rules(),
        )
        .unwrap();
        assert!(report.launch.hit);
        assert_eq!(report.heat_transfer, 0);
        assert!(report.narc.is_none() && report.cooling.is_none());
        match report.salvo.unwrap() {
            BattleTargetSalvo::Swarm(_) => panic!("Direct explosive ammunition cannot retarget"),
            BattleTargetSalvo::Mech(salvo) => assert_eq!(
                salvo.groups.iter().map(|group| group.damage).sum::<u16>(),
                10
            ),
            BattleTargetSalvo::Vehicle(salvo) => assert_eq!(
                salvo.groups.iter().map(|group| group.damage).sum::<u16>(),
                10
            ),
        }
        candidate.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_beacon_controls_share_native_lua_selection_and_rollback() {
    for (weapon, command, call, expected) in [
        (
            BattleWeapon::NarcBeacon,
            "explosive 0",
            "explosive",
            BattleAmmunitionMode::Narc,
        ),
        (
            BattleWeapon::Srm6,
            "narc 0",
            "narc",
            BattleAmmunitionMode::Narc,
        ),
        (
            BattleWeapon::INarcBeacon,
            "inarc 0 Y",
            "inarc",
            BattleAmmunitionMode::INarcHaywire,
        ),
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml")
            .replace("IS.AC/20", weapon.name())
            .replace("rounds = 5 }", "rounds = 4 }");
        let (_dir, config, base, _, [_, _, shooter, _]) = engagement(&template).await;
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
        let call = if call == "inarc" {
            format!("btech.unit.inarc({},1,0,'Y')", shooter.0)
        } else {
            format!("btech.unit.{call}({},1,0)", shooter.0)
        };
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&call).unwrap();
        let output = support::run_text(&native, &config, ObjectId(1), 1, command);
        assert!(output.contains("Weapon 0"), "{output}");
        assert_eq!(lua.world().btech, native.world().btech);
        assert_eq!(
            native.world().btech.vehicles()[&shooter]
                .ammunition_mode(0)
                .unwrap(),
            expected
        );
        native.world().validate(&config).unwrap();
        if weapon == BattleWeapon::INarcBeacon {
            assert!(
                support::run_text(&native, &config, ObjectId(1), 2, command)
                    .contains("already set")
            );
        }
    }
}

#[tokio::test]
async fn vehicle_targets_receive_pods_without_armor_damage_and_replay_interception() {
    for (weapon, mode, flag, kind) in [
        (
            BattleWeapon::NarcBeacon,
            BattleAmmunitionMode::Normal,
            "-",
            BattleBeaconKind::Narc,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::Normal,
            "-",
            BattleBeaconKind::Homing,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::INarcHaywire,
            "iNarc_Haywire",
            BattleBeaconKind::Haywire,
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::INarcEcm,
            "iNarc_ECM",
            BattleBeaconKind::Ecm,
        ),
    ] {
        let template = include_str!("../game/mechs/Demolisher.toml")
            .replace("IS.AC/20", weapon.name())
            .replace(
                "rounds = 5 }",
                &if flag == "-" {
                    "rounds = 4 }".to_owned()
                } else {
                    format!("rounds = 4, modes = [\"{flag}\"] }}")
                },
            );
        let (_dir, config, mut base, _, [_, _, shooter, target]) = engagement(&template).await;
        if weapon == BattleWeapon::INarcBeacon {
            set_battle_inarc_ammunition(&mut base, shooter, ObjectId(1), 0, mode).unwrap();
        }
        for (hit, defense) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut world = base.clone();
            if defense {
                install_vehicle_ams(&mut world, target, BattleWeapon::AntiMissileSystem);
            }
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            seed(&mut world, shooter, value);
            let before = world.btech.vehicles()[&target].clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let report =
                fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
                    .unwrap();
            assert_eq!(
                report,
                fire_battle_vehicle_shot(
                    &mut replay,
                    shooter,
                    ObjectId(1),
                    target,
                    0,
                    fire_rules()
                )
                .unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            let pod = report.narc.as_ref().unwrap();
            assert_eq!(
                (pod.kind, pod.hit, pod.intercepted),
                (kind, hit, hit && defense)
            );
            assert!(report.salvo.is_none());
            assert_eq!(
                world.btech.vehicles()[&target].sections(),
                before.sections()
            );
            assert_eq!(
                world.btech.vehicles()[&target].has_beacon(kind),
                hit && !defense
            );
            if hit && !defense {
                assert!(matches!(pod.section, Some(BattleUnitSection::Vehicle(_))));
            }
            assert_eq!(report.ams.is_some(), defense && hit);
            if defense && hit {
                assert_eq!(report.ams.as_ref().unwrap().shot_down, 1);
            }
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_beacons_drive_aim_guidance_interference_and_section_loss() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.SRM-6");
    let (_dir, config, mut world, _, [_, _, shooter, target]) = engagement(&template).await;
    toggle_battle_narc(&mut world, shooter, ObjectId(1), 0).unwrap();
    let before = battle_aim_modifiers(&world, shooter, target, 0, 6, rules()).unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["beacons"] = serde_json::json!({"turret":["haywire"]});
    saved["vehicles"][target.0.to_string()]["beacons"] =
        serde_json::json!({"turret":["homing","narc","ecm"]});
    world.btech = serde_json::from_value(saved).unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 6, rules())
            .unwrap()
            .beacon_accuracy,
        before.beacon_accuracy
    );
    assert!(
        battle_electronic_field(&world, target)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 7)
        .unwrap();
    seed(&mut world, target, value);
    let mut blocked = world.clone();
    let request = BattleVehicleSalvoRequest {
        range_damage: false,
        damage_penalty: 0,
        weapon: BattleWeapon::Lrm20,
        ammunition: BattleAmmunitionMode::Narc,
        fire_mode: BattleFireMode::Normal,
        gatling_damage: None,
        distance: 6.0,
        glancing: false,
        guidance_blocked: false,
        angel_blocked: false,
        intercepted: 0,
    };
    let enhanced = resolve_battle_vehicle_salvo(
        &mut world,
        target,
        BattleHitArc::Front,
        request,
        fire_rules().shot.vehicle_impact,
    )
    .unwrap();
    let ordinary = resolve_battle_vehicle_salvo(
        &mut blocked,
        target,
        BattleHitArc::Front,
        BattleVehicleSalvoRequest {
            range_damage: false,
            damage_penalty: 0,
            guidance_blocked: true,
            ..request
        },
        fire_rules().shot.vehicle_impact,
    )
    .unwrap();
    assert!(enhanced.missiles_before_defense.unwrap() > ordinary.missiles_before_defense.unwrap());
    for invalid in [
        serde_json::json!({"turret":[]}),
        serde_json::json!({"nose":["narc"]}),
    ] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][target.0.to_string()]["beacons"] = invalid;
        assert!(serde_json::from_value::<BtechState>(saved).is_err());
    }
    damage_battle_vehicle_phase(
        &mut world,
        target,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(world.btech.vehicles()[&target].beacons().is_empty());
    assert!(
        !battle_electronic_field(&world, target)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 6, rules())
            .unwrap()
            .beacon_accuracy,
        before.beacon_accuracy + 1
    );
    let mut corrupt = serde_json::to_value(&world.btech).unwrap();
    corrupt["vehicles"][target.0.to_string()]["beacons"] = serde_json::json!({"turret":["narc"]});
    assert!(serde_json::from_value::<BtechState>(corrupt).is_err());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_pod_host_action_rolls_back_attachment_and_location_effects() {
    let template =
        include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.NarcBeacon");
    let (_dir, config, mut world, _, [_, _, shooter, target]) = engagement(&template).await;
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut world, shooter, value);
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
    let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<()>(&call).unwrap();
    let output = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    assert!(output.contains("NARC Beacon attaches"), "{output}");
    assert_eq!(lua.world().btech, native.world().btech);
    assert!(
        lua.eval_callback::<bool>(&format!(
            "return next(btech.unit.state({}).beacons) ~= nil",
            target.0
        ))
        .unwrap()
    );
    lua.world().validate(&config).unwrap();
}

/// Seed attached effects without altering the vehicle's controls or equipment.
fn attach_removal_test_pods(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["beacons"] =
                serde_json::json!({"front":["narc"],"turret":["narc","homing","haywire","ecm"]});
        })
        .unwrap();
}

#[tokio::test]
async fn vehicle_pod_inspection_and_removal_share_controls_and_saved_countdown() {
    let (_dir, config, mut world, _, [target, _, shooter, _]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    assert!(
        inspect_battle_pods(&world, shooter, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    attach_removal_test_pods(&mut world, shooter);
    let rows = inspect_battle_pods(&world, shooter, ObjectId(1)).unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().any(|row| row.section
        == BattleUnitSection::Vehicle(BattleVehicleSection::Turret)
        && row.kinds.len() == 4));
    let before = world.btech.clone();
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
    let table = support::run_text(&native, &config, ObjectId(1), 1, "pods");
    assert!(
        table.contains("Front Side") && table.contains("Turret"),
        "{table}"
    );
    assert_eq!(
        lua.eval_callback::<usize>(&format!("return #btech.unit.pods({},1)", shooter.0))
            .unwrap(),
        5
    );
    assert_eq!(native.world().btech, before);
    let call = format!("btech.unit.removepods({},1)", shooter.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    assert!(
        lua.eval_callback::<bool>(&format!("return {call}"))
            .unwrap()
    );
    assert!(
        support::run_text(&native, &config, ObjectId(1), 2, "removepods")
            .contains("systematically remove")
    );
    assert_eq!(lua.world().btech, native.world().btech);
    world = lua.world().clone();
    assert_eq!(world.btech.vehicles()[&shooter].pod_removal(), Some(60));
    let pending = world.btech.clone();
    assert!(set_battle_speed(&mut world, shooter, ObjectId(1), 0.0).is_err());
    assert!(
        fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, 0, fire_rules())
            .is_err()
    );
    assert!(
        !world.btech.vehicles()[&shooter]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    assert_eq!(world.btech, pending);
    for _ in 0..59 {
        assert!(
            !advance_battle_units(&mut world, 0)
                .iter()
                .any(|notice| notice.text.contains("remove all the iNARC"))
        );
    }
    assert_eq!(world.btech.vehicles()[&shooter].pod_removal(), Some(1));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_units(&mut world, 0);
    assert_eq!(notices, advance_battle_units(&mut replay, 0));
    assert_eq!(world.btech, replay.btech);
    assert!(
        notices
            .iter()
            .any(|notice| notice.unit == shooter && notice.text.contains("remove all the iNARC"))
    );
    assert_eq!(world.btech.vehicles()[&shooter].pod_removal(), None);
    assert!(world.btech.vehicles()[&shooter].has_beacon(BattleBeaconKind::Narc));
    for kind in [
        BattleBeaconKind::Homing,
        BattleBeaconKind::Haywire,
        BattleBeaconKind::Ecm,
    ] {
        assert!(!world.btech.vehicles()[&shooter].has_beacon(kind));
    }
    assert!(
        !battle_electronic_field(&world, shooter)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    set_battle_speed(&mut world, shooter, ObjectId(1), 1.0).unwrap();
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_pod_removal_guards_shutdown_and_destruction_preserve_action_boundaries() {
    let (_dir, config, mut base, _, [_, _, shooter, _]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    attach_removal_test_pods(&mut base, shooter);
    for condition in [
        "wrong_pilot",
        "off",
        "speed",
        "desired",
        "stun",
        "turret",
        "unjam",
        "narc_only",
        "duplicate",
    ] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                let unit = record;
                match condition {
                    "off" => unit["power"] = serde_json::to_value(BattlePower::Off).unwrap(),
                    "speed" => unit["motion"]["speed"] = 1.0.into(),
                    "desired" => unit["motion"]["desired_speed"] = 1.0.into(),
                    "stun" => unit["crew_stun_remaining"] = 10.into(),
                    "turret" => {
                        unit["turret_jammed"] = true.into();
                        unit["turret_repairs"] = serde_json::json!([60]);
                    }
                    "unjam" => unit["unjam"] = serde_json::json!({"weapon_index":0,"remaining":60}),
                    "narc_only" => unit["beacons"] = serde_json::json!({"front":["narc"]}),
                    "duplicate" => unit["pod_removal"] = 60.into(),
                    _ => {}
                }
            })
            .unwrap();
        let before = world.btech.clone();
        assert!(
            begin_battle_pod_removal(
                &mut world,
                shooter,
                ObjectId(if condition == "wrong_pilot" { 2 } else { 1 })
            )
            .is_err(),
            "{condition}"
        );
        assert_eq!(world.btech, before, "{condition}");
    }
    for remaining in [0, 61] {
        let mut saved = serde_json::to_value(&base.btech).unwrap();
        saved["vehicles"][shooter.0.to_string()]["pod_removal"] = remaining.into();
        assert!(serde_json::from_value::<BtechState>(saved).is_err());
    }
    for destroyed in [false, true] {
        let mut world = base.clone();
        begin_battle_pod_removal(&mut world, shooter, ObjectId(1)).unwrap();
        if destroyed {
            damage_battle_vehicle_phase(
                &mut world,
                shooter,
                BattleVehicleSection::Front,
                100,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        } else {
            power(&mut world, &[shooter], BattlePower::Off);
        }
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                record["pod_removal"] = 1.into();
            })
            .unwrap();
        let notices = advance_battle_units(&mut world, 0);
        assert_eq!(world.btech.vehicles()[&shooter].pod_removal(), None);
        assert_eq!(
            notices.iter().any(
                |notice| notice.unit == shooter && notice.text.contains("remove all the iNARC")
            ),
            !destroyed
        );
        assert_eq!(
            world.btech.vehicles()[&shooter].has_beacon(BattleBeaconKind::Ecm),
            destroyed
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn shutdown_vehicle_pod_expiry_retries_failed_server_commit() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, _, ids) = engagement(include_str!("../game/mechs/Demolisher.toml")).await;
        let shooter = ids[2];
        attach_removal_test_pods(&mut world, shooter);
        begin_battle_pod_removal(&mut world, shooter, ObjectId(1)).unwrap();
        power(&mut world, &ids, BattlePower::Off);
        world.btech
            .rewrite_unit_record(shooter, |record| {
        record["pod_removal"] = 1.into();
        })
            .unwrap();
        assert!(battle_contact_observers(&world).is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_pods BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'pod removal failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech.vehicles()[&shooter].pod_removal(), Some(1));
        assert!(saved.btech.vehicles()[&shooter].has_beacon(BattleBeaconKind::Ecm));
        sqlx::raw_sql("DROP TRIGGER deny_pods").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.btech.vehicles()[&shooter].pod_removal().is_none()).await;
        assert!(!saved.btech.vehicles()[&shooter].has_beacon(BattleBeaconKind::Ecm));
        assert!(saved.btech.vehicles()[&shooter].has_beacon(BattleBeaconKind::Narc));
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}

/// Put the same pilot in a Mech facing the vehicle target used by the mixed firing scenarios.
async fn mech_engagement() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world, map, [shooter, _, previous, target]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    release_battle_pilot(&mut world, previous, ObjectId(1)).unwrap();
    power(&mut world, &[shooter], BattlePower::Off);
    place_battle_unit(&mut world, shooter, map, 0, 1).unwrap();
    power(&mut world, &[shooter], BattlePower::Running);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[shooter, target]).unwrap();
    (dir, config, world, shooter, target)
}

/// Conventional Mech attacks use vehicle damage, target-owned dice and the ordinary saved lock.
#[tokio::test]
async fn mech_vehicle_fire_locks_damage_and_restart_share_existing_resolvers() {
    let (_dir, config, base, shooter, target) = mech_engagement().await;
    for hit in [false, true] {
        let mut world = base.clone();
        let value = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 })
            .unwrap();
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
            })
            .unwrap();
        select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
        for _ in 0..3 {
            advance_battle_target_locks(&mut world);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 0..5 {
            assert_eq!(
                advance_battle_target_locks(&mut world),
                advance_battle_target_locks(&mut restored)
            );
        }
        let before = world.btech.vehicles()[&target].clone();
        let report = resolve_battle_shot(
            &mut world,
            shooter,
            ObjectId(1),
            target,
            0,
            fire_rules().shot,
        )
        .unwrap();
        assert_eq!(
            report,
            resolve_battle_shot(
                &mut restored,
                shooter,
                ObjectId(1),
                target,
                0,
                fire_rules().shot
            )
            .unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(report.salvo.is_some(), hit);
        assert_eq!(report.aim.target_lock, 0);
        if hit {
            let Some(BattleTargetSalvo::Vehicle(salvo)) = report.salvo else {
                panic!("Expected vehicle damage")
            };
            assert_eq!(
                salvo.groups.iter().map(|group| group.damage).sum::<u16>(),
                5
            );
            assert_ne!(
                world.btech.vehicles()[&target].sections(),
                before.sections()
            );
        } else {
            assert_eq!(&world.btech.vehicles()[&target], &before);
        }
        world.validate(&config).unwrap();
    }
}

/// The native and Lua routes publish one shared result and restore locks, dice and damage on abort.
#[tokio::test]
async fn mech_vehicle_native_and_lua_fire_are_atomic_and_use_target_selection() {
    let (_dir, config, mut world, shooter, target) = mech_engagement().await;
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["constructed"][shooter.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    // Keep this command/cooldown fixture independent of random target criticals.
    let target_seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() == 7 && dice.two_d6() == 7
        })
        .unwrap();
    saved["vehicles"][target.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([target_seed; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
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
    let call = format!("btech.unit.fire({},1,0)", shooter.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let output = support::run_text(&native, &config, ObjectId(1), 1, "fire 0");
    assert!(
        output.contains("You fire IS.MediumLaser") && output.contains("Hit."),
        "{output}"
    );
    let kind: String = lua
        .eval_callback(&format!("return {call}.salvo.kind"))
        .unwrap();
    assert_eq!(kind, "vehicle");
    assert_eq!(lua.world().btech, native.world().btech);
    let before = native.world().btech.clone();
    let output = support::run_text(&native, &config, ObjectId(1), 1, "fire 0");
    assert!(output.contains("still recharging"), "{output}");
    assert_eq!(native.world().btech, before);
    native.world().validate(&config).unwrap();
}

/// Add one launcher with its own intact feed, leaving the scenario's movement and crew unchanged.
fn install_mech_launcher(
    world: &mut World,
    shooter: ObjectId,
    weapon: BattleWeapon,
    mode: BattleAmmunitionMode,
) -> usize {
    let mut definition = world.btech.constructed_units()[&shooter]
        .definition()
        .clone();
    let section = definition
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    for slot in 2..2 + weapon.profile().critical_slots {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    section.criticals.insert(
        10,
        CriticalDefinition {
            equipment: format!("Ammo_{}", weapon.name()),
            data: "4".into(),
            modes: if mode == BattleAmmunitionMode::INarcEcm {
                vec!["iNarc_ECM".into()]
            } else {
                vec![]
            },
        },
    );
    let rebuilt = BattleUnit::from_template(definition.clone()).unwrap();
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"] = serde_json::to_value(rebuilt.ammunition()).unwrap();
        })
        .unwrap();
    world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == weapon)
        .unwrap()
}

/// Mech-launched missiles and pods share vehicle AMS, packet grouping and beacon attachment.
#[tokio::test]
async fn mech_vehicle_missiles_and_beacons_share_defenses_and_target_effects() {
    let (_dir, config, base, shooter, target) = mech_engagement().await;
    for (weapon, mode, kind) in [
        (BattleWeapon::Srm6, BattleAmmunitionMode::Normal, None),
        (
            BattleWeapon::NarcBeacon,
            BattleAmmunitionMode::Normal,
            Some(BattleBeaconKind::Narc),
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::Normal,
            Some(BattleBeaconKind::Homing),
        ),
        (
            BattleWeapon::INarcBeacon,
            BattleAmmunitionMode::INarcEcm,
            Some(BattleBeaconKind::Ecm),
        ),
    ] {
        for defended in [false, true] {
            for hit in [false, true] {
                let mut world = base.clone();
                let index = install_mech_launcher(&mut world, shooter, weapon, mode);
                if defended {
                    install_vehicle_ams(&mut world, target, BattleWeapon::AntiMissileSystem);
                }
                let value = (0..=255)
                    .find(|value| {
                        BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 }
                    })
                    .unwrap();
                world
                    .btech
                    .rewrite_unit_record(shooter, |record| {
                        record["dice"] =
                            serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
                        if mode != BattleAmmunitionMode::Normal {
                            record["ammunition_modes"][index.to_string()] =
                                serde_json::to_value(mode).unwrap();
                        }
                    })
                    .unwrap();
                let before = world.clone();
                let report = resolve_battle_shot(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    target,
                    index,
                    fire_rules().shot,
                )
                .unwrap_or_else(|error| {
                    panic!("{weapon:?} {mode:?} defended={defended} hit={hit}: {error:#}")
                });
                assert_eq!(report.ams.is_some(), defended && hit);
                if let Some(kind) = kind {
                    assert!(report.salvo.is_none());
                    let pod = report.narc.unwrap();
                    assert_eq!(pod.hit, hit);
                    assert_eq!(pod.intercepted, defended && hit);
                    assert_eq!(pod.section.is_some(), hit && !defended);
                    assert_eq!(
                        world.btech.vehicles()[&target].has_beacon(kind),
                        hit && !defended
                    );
                    assert_eq!(
                        world.btech.vehicles()[&target].sections(),
                        before.btech.vehicles()[&target].sections()
                    );
                } else if hit {
                    let Some(BattleTargetSalvo::Vehicle(salvo)) = report.salvo else {
                        panic!("Expected vehicle salvo")
                    };
                    let intercepted = report.ams.as_ref().map_or(0, |ams| ams.shot_down);
                    assert_eq!(
                        salvo.groups.iter().map(|group| group.damage).sum::<u16>(),
                        u16::from(salvo.missiles_before_defense.unwrap() - intercepted) * 2
                    );
                } else {
                    assert!(report.salvo.is_none());
                }
                world.validate(&config).unwrap();
            }
        }
    }
}

/// Target admission is atomic, and vehicle placement clears a Mech's stale selection.
#[tokio::test]
async fn mech_vehicle_admission_hex_selection_and_lock_cleanup() {
    let (_dir, config, base, shooter, target) = mech_engagement().await;
    for case in ["wrong_pilot", "off", "character_source", "character_target"] {
        let mut world = base.clone();
        let index = 0;
        let pilot = if case == "wrong_pilot" {
            ObjectId(2)
        } else {
            ObjectId(1)
        };
        match case {
            "off" => power(&mut world, &[shooter], BattlePower::Off),
            "character_source" => {
                world
                    .objects
                    .get_mut(&shooter)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            "character_target" => {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            _ => {}
        }
        let before = world.btech.clone();
        assert!(
            resolve_battle_shot(&mut world, shooter, pilot, target, index, fire_rules().shot)
                .is_err(),
            "{case}"
        );
        assert_eq!(world.btech, before, "{case}");
    }
    let mut world = base;
    select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
    let map = world.btech.vehicles()[&target].position().unwrap().map;
    power(&mut world, &[target], BattlePower::Off);
    place_battle_unit(&mut world, target, map, 0, 0).unwrap();
    assert!(
        world.btech.constructed_units()[&shooter]
            .target_lock()
            .is_none()
    );
    power(&mut world, &[target], BattlePower::Running);
    let other: Vec<_> = world
        .btech
        .constructed_units()
        .keys()
        .copied()
        .filter(|id| *id != shooter)
        .collect();
    for id in other {
        power(&mut world, &[id], BattlePower::Off);
        place_battle_unit(&mut world, id, map, 0, 4).unwrap();
    }
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let hex = BattleHexCoordinate { x: 0, y: 0 };
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        hex,
        BattleHexTargetMode::UnitAtHex,
    )
    .unwrap();
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 2)
        .unwrap();
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        })
        .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let (recipient, x, y): (i64, i32, i32) = scripts
        .eval_callback(&format!(
            "local r=btech.unit.fire({},1,0); return r.target,r.coordinate.x,r.coordinate.y",
            shooter.0
        ))
        .unwrap();
    assert_eq!((recipient, x, y), (target.0, 0, 0));
    scripts.world().validate(&config).unwrap();
}

/// An equipped shooter of either class, sharing the same vehicle recipient geometry.
async fn weapon_engagement(
    vehicle: bool,
    weapon: BattleWeapon,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    if vehicle {
        let template =
            include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", weapon.name());
        let (dir, config, world, _, [_, _, shooter, target]) = engagement(&template).await;
        return (dir, config, world, shooter, target, 0);
    }
    let (dir, config, mut world, shooter, target) = mech_engagement().await;
    let index = install_mech_launcher(&mut world, shooter, weapon, BattleAmmunitionMode::Normal);
    (dir, config, world, shooter, target, index)
}

/// Inspect complete outcomes through the common serialized report vocabulary.
fn tactical_shot(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    index: usize,
    rules: BattleVehicleShotRules,
) -> serde_json::Value {
    if world.btech.vehicles().contains_key(&shooter) {
        return serde_json::to_value(
            fire_battle_vehicle_shot(world, shooter, ObjectId(1), target, index, rules).unwrap(),
        )
        .unwrap();
    }
    serde_json::to_value(
        resolve_battle_shot(world, shooter, ObjectId(1), target, index, rules.shot).unwrap(),
    )
    .unwrap()
}

/// IS plasma heats Mechs only; vehicle impacts use catalogue damage without an extra thermal draw.
#[tokio::test]
async fn plasma_vehicle_targets_share_damage_glancing_misses_and_replay() {
    for vehicle in [false, true] {
        let (_dir, config, base, shooter, target, index) =
            weapon_engagement(vehicle, BattleWeapon::PlasmaRifle).await;
        for damage in [0u32, 5, 10] {
            let mut world = base.clone();
            let mut rules = fire_rules();
            rules.shot.glancing = if damage == 5 {
                BattleGlancingMode::AtTarget
            } else {
                BattleGlancingMode::Disabled
            };
            let threshold =
                battle_pilot_aim_modifiers(&world, shooter, target, index, false, rules.shot.aim)
                    .unwrap()
                    .subtotal()
                    .unwrap();
            let roll = match damage {
                0 => 2,
                5 => threshold as u8,
                _ => 12,
            };
            let value = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == roll)
                .unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved[if vehicle { "vehicles" } else { "constructed" }][shooter.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
            saved["vehicles"][target.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([17; 32])).unwrap();
            world.btech = serde_json::from_value(saved).unwrap();
            let mut expected = world.clone();
            if damage > 0 {
                let direction = battle_unit_range(&expected, target, shooter).unwrap();
                let heading = expected.btech.vehicles()[&target].motion().unwrap().heading;
                let arc = BattleHitArc::from_bearing(
                    direction.bearing.unwrap_or(180.0),
                    heading,
                    rules.shot.hit_arc_mode,
                )
                .unwrap();
                let impact = resolve_battle_vehicle_impact(
                    &mut expected,
                    target,
                    arc,
                    damage,
                    None,
                    rules.shot.vehicle_impact,
                )
                .unwrap();
                assert!(impact.damage.is_some());
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let report = tactical_shot(&mut world, shooter, target, index, rules);
            assert_eq!(
                report,
                tactical_shot(&mut replay, shooter, target, index, rules)
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(
                world.btech.vehicles()[&target],
                expected.btech.vehicles()[&target]
            );
            assert_eq!(report["heat_transfer"], 0);
            assert!(report["narc"].is_null() && report["cooling"].is_null());
            if damage == 0 {
                assert!(report["salvo"].is_null());
            } else {
                assert_eq!(report["salvo"]["kind"], "vehicle");
                let groups = report["salvo"]["report"]["groups"].as_array().unwrap();
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0]["damage"], damage);
            }
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn plasma_vehicle_targets_share_native_lua_feedback_and_rollback() {
    for vehicle in [false, true] {
        let (_dir, config, mut world, shooter, target, index) =
            weapon_engagement(vehicle, BattleWeapon::PlasmaRifle).await;
        let value = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[if vehicle { "vehicles" } else { "constructed" }][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
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
        let call = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        assert!(
            text.contains("You fire IS.PlasmaRifle") && text.contains("Hit."),
            "{text}"
        );
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}.salvo.kind == 'vehicle'"))
                .unwrap()
        );
        assert_eq!(lua.world().btech, native.world().btech);
        lua.world().validate(&config).unwrap();
    }
}

/// Coolant credit and direct flamer heat have identical recipients regardless of carrier anatomy.
#[tokio::test]
async fn thermal_vehicle_targets_share_replay_feedback_and_rollback() {
    for carrier in [false, true] {
        for weapon in [
            BattleWeapon::CoolantGun,
            BattleWeapon::VehicleFlamer,
            BattleWeapon::VehicleHeavyFlamer,
        ] {
            let (_dir, config, base, shooter, target, index) =
                weapon_engagement(carrier, weapon).await;
            for hit in [false, true] {
                let mut world = base.clone();
                if weapon != BattleWeapon::CoolantGun {
                    toggle_battle_flamer_heat(&mut world, shooter, ObjectId(1), index).unwrap();
                }
                let seed = (0..=255)
                    .find(|value| {
                        BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 }
                    })
                    .unwrap();
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved[if carrier { "vehicles" } else { "constructed" }][shooter.0.to_string()]["dice"] =
                    serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                saved["vehicles"][target.0.to_string()]["weapon_heat"] = 1.5.into();
                world.btech = serde_json::from_value(saved).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                let lua = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(replay.clone())),
                )
                .unwrap();
                let call = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
                let report = tactical_shot(&mut world, shooter, target, index, fire_rules());
                assert_eq!(
                    report,
                    tactical_shot(&mut replay, shooter, target, index, fire_rules())
                );
                assert_eq!(world.btech, replay.btech);
                let delta = if !hit {
                    0.0
                } else if weapon == BattleWeapon::CoolantGun {
                    -f64::from(weapon.profile().damage)
                } else {
                    f64::from(weapon.profile().damage)
                };
                assert_eq!(world.btech.vehicles()[&target].weapon_heat(), 1.5 + delta);
                assert_eq!(
                    report["cooling"],
                    if hit && weapon == BattleWeapon::CoolantGun {
                        serde_json::json!(weapon.profile().damage as f64)
                    } else {
                        serde_json::Value::Null
                    }
                );
                assert_eq!(
                    report["heat_transfer"],
                    if hit && weapon != BattleWeapon::CoolantGun {
                        weapon.profile().damage
                    } else {
                        0
                    }
                );
                assert!(report["salvo"].is_null());
                lua.eval_callback::<()>(&call).unwrap();
                let output = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                assert!(output.contains("You fire"), "{output}");
                assert_eq!(native.world().btech, world.btech);
                assert_eq!(lua.world().btech, world.btech);
                for _ in 0..60 {
                    let _ = advance_battle_units(&mut world, 0);
                    let _ = advance_battle_heat(&mut world);
                }
                assert_eq!(world.btech.vehicles()[&target].weapon_heat(), 1.5 + delta);
                world.validate(&config).unwrap();
            }
        }
    }
}

/// Self-cooling bypasses target selection and optical acquisition but still reserves a real shot.
#[tokio::test]
async fn vehicle_self_cooling_shares_host_selection_and_atomic_expenditure() {
    let (_dir, config, mut base, shooter, target, index) =
        weapon_engagement(true, BattleWeapon::CoolantGun).await;
    toggle_battle_flamer_heat(&mut base, shooter, ObjectId(1), index).unwrap();
    let seed_value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut base, shooter, seed_value);
    base.btech
        .rewrite_unit_record(shooter, |record| {
            record["contacts"] = serde_json::json!({});
            record["weapon_heat"] = 1.5.into();
        })
        .unwrap();
    for selection in 0..4 {
        let explicit = selection == 1 || selection == 3;
        let recipient = if selection == 1 { target } else { shooter };
        let mut base = base.clone();
        if selection == 1 {
            refresh_battle_contacts(&mut base, &[shooter]).unwrap();
        }
        if selection == 2 {
            select_battle_hex_target(
                &mut base,
                shooter,
                ObjectId(1),
                BattleHexCoordinate { x: 0, y: 4 },
                BattleHexTargetMode::Hex,
            )
            .unwrap();
        }
        let mut raw = base.clone();
        let report = fire_battle_vehicle_shot(
            &mut raw,
            shooter,
            ObjectId(1),
            recipient,
            index,
            fire_rules(),
        )
        .unwrap();
        assert_eq!(report.target, recipient);
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
        let call = if explicit {
            format!("btech.unit.fire({},1,{index},{})", shooter.0, recipient.0)
        } else {
            format!("btech.unit.fire({},1,{index})", shooter.0)
        };
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        let (actual_recipient, cooling): (i64, f64) = lua
            .eval_callback(&format!("local r={call}; return r.target,r.cooling"))
            .unwrap();
        assert_eq!(actual_recipient, recipient.0);
        assert_eq!(
            cooling,
            f64::from(BattleWeapon::CoolantGun.profile().damage)
        );
        let command = if explicit {
            format!("fire {index} #{}", recipient.0)
        } else {
            format!("fire {index}")
        };
        let output = support::run_text(&native, &config, ObjectId(1), 1, &command);
        assert!(
            output.contains(if recipient == shooter {
                "Coolant washes"
            } else {
                "stream of coolant"
            }),
            "{output}"
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            lua.world().btech.vehicles()[&shooter].weapon_heat(),
            1.5 - if recipient == shooter { cooling } else { 0.0 }
        );
        if recipient == shooter {
            assert_eq!(
                lua.world().btech.vehicles()[&target],
                base.btech.vehicles()[&target]
            );
        } else {
            assert_eq!(
                lua.world().btech.vehicles()[&target].weapon_heat(),
                base.btech.vehicles()[&target].weapon_heat() - cooling
            );
        }
        let committed = lua.world().btech.clone();
        assert!(lua.eval_callback::<()>(&call).is_err());
        assert_eq!(lua.world().btech, committed);
        let mut empty = base.clone();
        empty
            .btech
            .rewrite_unit_record(shooter, |record| {
                for rounds in record["ammunition"].as_array_mut().unwrap() {
                    *rounds = 0.into();
                }
            })
            .unwrap();
        let before = empty.btech.clone();
        assert!(
            fire_battle_vehicle_shot(
                &mut empty,
                shooter,
                ObjectId(1),
                target,
                index,
                fire_rules()
            )
            .is_err()
        );
        assert_eq!(empty.btech, before);
        lua.world().validate(&config).unwrap();
    }
}

/// Either launcher carrier reaches the same inferno recipient path after shared cluster and AMS rules.
#[tokio::test]
async fn inferno_shots_share_vehicle_burning_defenses_and_host_rollback() {
    for carrier in [false, true] {
        for advanced_fire in [false, true] {
            let (dir, _config, mut base, shooter, target, index) =
                weapon_engagement(carrier, BattleWeapon::Srm6).await;
            let path = dir.path().join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("fasaadvvhlfire".into(), i64::from(advanced_fire).into());
            std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            let class = if carrier { "vehicles" } else { "constructed" };
            let mut saved = serde_json::to_value(&base.btech).unwrap();
            for section in saved[class][shooter.0.to_string()]["definition"]["sections"]
                .as_object_mut()
                .unwrap()
                .values_mut()
            {
                for critical in section["criticals"].as_object_mut().unwrap().values_mut() {
                    if critical["equipment"]
                        .as_str()
                        .is_some_and(|name| name == "Ammo_IS.SRM-6")
                    {
                        critical["modes"] = serde_json::json!(["Inferno"]);
                    }
                }
            }
            base.btech = serde_json::from_value(saved).unwrap();
            let native_mode = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
            )
            .unwrap();
            let lua_mode = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
            )
            .unwrap();
            let mode_call = format!("btech.unit.inferno({},1,{index})", shooter.0);
            assert!(
                lua_mode
                    .eval_callback::<()>(&format!("{mode_call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua_mode.world().btech, base.btech);
            assert!(lua_mode.drain_outbox().is_empty());
            lua_mode.eval_callback::<()>(&mode_call).unwrap();
            let output = support::run_text(
                &native_mode,
                &config,
                ObjectId(1),
                1,
                &format!("inferno {index}"),
            );
            assert!(output.contains("set to fire Inferno"), "{output}");
            assert_eq!(native_mode.world().btech, lua_mode.world().btech);
            base = lua_mode.world().clone();
            for hit in [false, true] {
                for defense in [false, true] {
                    let mut world = base.clone();
                    if defense {
                        install_vehicle_ams(
                            &mut world,
                            target,
                            BattleWeapon::ClanAntiMissileSystem,
                        );
                    }
                    let value = (0..=255)
                        .find(|value| {
                            BattleDice::seeded([*value; 32]).two_d6() == if hit { 12 } else { 2 }
                        })
                        .unwrap();
                    let mut saved = serde_json::to_value(&world.btech).unwrap();
                    saved[class][shooter.0.to_string()]["dice"] =
                        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
                    saved["vehicles"][target.0.to_string()]["dice"] =
                        serde_json::to_value(BattleDice::seeded([17; 32])).unwrap();
                    world.btech = serde_json::from_value(saved).unwrap();
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
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
                    let call = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
                    assert!(
                        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                            .is_err()
                    );
                    assert_eq!(lua.world().btech, world.btech);
                    assert!(lua.drain_outbox().is_empty());
                    let mut rules = fire_rules();
                    rules.shot.vehicle_impact.advanced_fire = advanced_fire;
                    let report = tactical_shot(&mut world, shooter, target, index, rules);
                    assert_eq!(
                        report,
                        tactical_shot(&mut restored, shooter, target, index, rules)
                    );
                    assert_eq!(world.btech, restored.btech);
                    if hit {
                        let salvo = &report["salvo"]["report"];
                        assert!(salvo["groups"].as_array().unwrap().is_empty());
                        let hits = salvo["missiles_before_defense"].as_u64().unwrap();
                        let intercepted = report["ams"]["shot_down"].as_u64().unwrap_or(0);
                        let remaining = hits - intercepted;
                        if remaining > 0 {
                            assert_eq!(salvo["inferno"]["missiles"], remaining);
                            if advanced_fire {
                                assert_eq!(
                                    world.btech.vehicles()[&target].burning_sections().len(),
                                    5
                                );
                                assert_eq!(
                                    visible_battle_contact(&world, shooter, target)
                                        .unwrap()
                                        .unwrap()
                                        .status
                                        .chars()
                                        .nth(3),
                                    Some('B')
                                );
                            } else {
                                assert!(salvo["inferno"]["explosion_roll"].as_u64().is_some());
                            }
                        } else {
                            assert!(salvo["inferno"].is_null());
                        }
                    } else {
                        assert!(report["salvo"].is_null());
                    }
                    lua.eval_callback::<()>(&call).unwrap();
                    let output = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("fire {index} #{}", target.0),
                    );
                    assert!(output.contains("You fire"), "{output}");
                    assert_eq!(lua.world().btech, world.btech);
                    assert_eq!(native.world().btech, world.btech);
                    world.validate(&config).unwrap();
                }
            }
        }
    }
}

#[tokio::test]
async fn character_direct_fire_shares_native_lua_casualties_for_both_chassis() {
    use std::{cell::RefCell, rc::Rc};
    for classic in [false, true] {
        for vehicle_shooter in [false, true] {
            for vehicle_target in [false, true] {
                let (_dir, _config, mut world, map, ids) =
                    engagement(include_str!("../game/mechs/Demolisher.toml")).await;
                let path = _dir.path().join("stompymux.toml");
                if classic {
                    let text = std::fs::read_to_string(&path)
                        .unwrap()
                        .replace("oldxpsystem = 0", "oldxpsystem = 1");
                    std::fs::write(&path, text).unwrap();
                }
                let config = Config::load(_dir.path()).unwrap();
                let shooter = if vehicle_shooter { ids[2] } else { ids[0] };
                let target = if vehicle_target { ids[3] } else { ids[1] };
                if !vehicle_shooter {
                    release_battle_pilot(&mut world, ids[2], ObjectId(1)).unwrap();
                    power(&mut world, &[shooter], BattlePower::Off);
                    place_battle_unit(&mut world, shooter, map, 0, 1).unwrap();
                    power(&mut world, &[shooter], BattlePower::Running);
                    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
                    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
                }
                for id in [shooter, target] {
                    world
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                }
                world
                    .objects
                    .get_mut(&ObjectId(2))
                    .unwrap()
                    .flags
                    .remove(Flag::Wizard);
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
                set_battle_character(
                    &mut world,
                    ObjectId(2),
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
                assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
                if vehicle_target {
                    injure_battle_character_pilot(&mut world, target, 9, false).unwrap();
                }
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                let hit_seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                saved[if vehicle_shooter {
                    "vehicles"
                } else {
                    "constructed"
                }][shooter.0.to_string()]["dice"] =
                    serde_json::to_value(BattleDice::seeded([hit_seed; 32])).unwrap();
                if vehicle_target {
                    for section in saved["vehicles"][target.0.to_string()]["sections"]
                        .as_object_mut()
                        .unwrap()
                        .values_mut()
                    {
                        section["armor"] = 1.into();
                    }
                }
                if !vehicle_target {
                    let head_seed = (0..=255)
                        .find(|seed| {
                            let mut dice = BattleDice::seeded([*seed; 32]);
                            dice.two_d6() == 12
                        })
                        .unwrap();
                    saved["constructed"][target.0.to_string()]["dice"] =
                        serde_json::to_value(BattleDice::seeded([head_seed; 32])).unwrap();
                    saved["constructed"][target.0.to_string()]["sections"]["Head"]["armor"] =
                        0.into();
                    saved["constructed"][target.0.to_string()]["sections"]["Head"]["internal"] =
                        1.into();
                }
                world.btech = serde_json::from_value(saved).unwrap();
                if vehicle_target {
                    let mut impact_rules = BattleVehicleImpactRules::STANDARD;
                    impact_rules.criticals.table = BattleVehicleCriticalTable::from_settings(
                        config.battletech.fasaadvvhlcrit != 0,
                    );
                    impact_rules.hit.critical_mode = config.battletech.vcrit;
                    impact_rules.hit.critical_level = config.battletech.critlevel;
                    let seed = (0..=255)
                        .find(|seed| {
                            let mut probe = world.clone();
                            probe
                                .btech
                                .rewrite_unit_record(target, |record| {
                                    record["dice"] =
                                        serde_json::to_value(BattleDice::seeded([*seed; 32]))
                                            .unwrap();
                                })
                                .unwrap();
                            let _ = resolve_battle_vehicle_impact(
                                &mut probe,
                                target,
                                BattleHitArc::Rear,
                                if vehicle_shooter { 20 } else { 5 },
                                None,
                                impact_rules,
                            )
                            .unwrap();
                            probe.btech.vehicles()[&target]
                                .character_pilot_status()
                                .is_some_and(|status| status.killed)
                        })
                        .unwrap();
                    world
                        .btech
                        .rewrite_unit_record(target, |record| {
                            record["dice"] =
                                serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                        })
                        .unwrap();
                }
                refresh_battle_contacts(&mut world, &[shooter]).unwrap();
                select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
                set_battle_unit_signature(
                    &mut world,
                    target,
                    BattleUnitSignature {
                        team: 1,
                        ..Default::default()
                    },
                )
                .unwrap();
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
                for skill in ["Gunnery-Laser", "Gunnery-Ballistic", "Gunnery-Battlemech"] {
                    set_battle_character_value(
                        &mut world,
                        ObjectId(1),
                        skill,
                        BattleCharacterValue {
                            value: 2,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                }

                set_battle_unit_experience(
                    &mut world,
                    shooter,
                    BattleUnitExperience {
                        multiplier: 100.0,
                        ..Default::default()
                    },
                )
                .unwrap();
                let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let call = format!("btech.unit.fire({},1,0)", shooter.0);
                let afterlife = ObjectId(config.battletech.afterlife_dbref);
                lua.world_mut().objects.remove(&afterlife);
                assert!(lua.eval_callback::<()>(&call).is_err());
                assert_eq!(lua.world().btech, world.btech);
                assert_eq!(lua.world().objects[&ObjectId(2)].location, Some(target));
                *lua.world_mut() = world.clone();
                let started = chrono::Utc::now().timestamp();
                let output = support::run_text(&native, &config, ObjectId(1), 1, "fire 0");
                assert!(
                    output.contains("Hit."),
                    "{vehicle_shooter}/{vehicle_target}: {output}"
                );
                lua.eval_callback::<()>(&call).unwrap();
                let finished = chrono::Utc::now().timestamp();
                // Separate wall-clock award times from deterministic gameplay state.
                let mut states = [
                    serde_json::to_value(&lua.world().btech).unwrap(),
                    serde_json::to_value(&native.world().btech).unwrap(),
                ];
                for state in &mut states {
                    for value in state["character_values"]["1"]
                        .as_object_mut()
                        .unwrap()
                        .values_mut()
                    {
                        let timestamp = value["last_used"].as_i64().unwrap();
                        if timestamp != 0 {
                            assert!((started..=finished).contains(&timestamp));
                            value["last_used"] = 0.into();
                        }
                    }
                }
                assert_eq!(states[0], states[1]);
                assert_eq!(lua.world().objects[&ObjectId(2)].location, Some(afterlife));
                let result = lua.world().clone();
                assert!(
                    result.btech.character_values()[&ObjectId(1)]
                        .iter()
                        .any(|(name, value)| name.starts_with("Gunnery-")
                            && value.experience_balance() > 0),
                    "missing XP: {classic}/{vehicle_shooter}/{vehicle_target}"
                );
                persistence::save(&config.database(), &result)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    result.btech
                );
            }
        }
    }
}

#[tokio::test]
async fn vehicle_missile_packets_award_experience_inside_the_firing_transaction() {
    use std::{cell::RefCell, rc::Rc};
    let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", "IS.LRM-20");
    let (_dir, config, mut world, _map, ids) = engagement(&template).await;
    let [_, _, shooter, target] = ids;
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    for id in [shooter, target] {
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
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
    for skill in [
        "Gunnery-Missile",
        "Gunnery-Battlemech",
        "Gunnery-Conventional",
    ] {
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            skill,
            BattleCharacterValue {
                value: 4,
                ..Default::default()
            },
        )
        .unwrap();
    }
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][shooter.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    // Clustering uses the victim's stream; fix both rolls for the multi-packet case.
    saved["vehicles"][target.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    saved["vehicles"][target.0.to_string()]["definition"]["attributes"]["specials"] =
        "ICEEngine_Tech CritProof_Tech".into();
    world.btech = serde_json::from_value(saved).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let call = format!("local r=btech.unit.fire({},1,0,{});", shooter.0, target.0);
    assert!(
        scripts
            .eval_callback::<()>(&(call.clone() + " error('abort')"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    let (groups, awards, complete): (usize, usize, bool) = scripts.eval_callback(&(call + " if not r.salvo then local fields={}; for k,v in pairs(r.aim) do fields[#fields+1]=k..'='..tostring(v) end; error(table.concat(fields,',')..' roll='..tostring(r.launch.roll)) end; local s=r.salvo.report; local complete=true; for _,a in ipairs(s.experience) do complete=complete and a.formula=='battle_value' and a.award.accepted end; return #s.groups,#s.experience,complete")).unwrap();
    assert!(groups >= 2);
    assert_eq!(awards, groups);
    assert!(complete);
    assert!(
        scripts.world().btech.character_values()[&ObjectId(1)]
            .values()
            .any(|value| value.experience_balance() > 0)
    );
    let result = scripts.world().clone();
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        scripts.world().btech
    );
}

/// Vehicle shooters use the same attributed Mech-target cascade as Mech shooters.
#[tokio::test]
async fn weapons_hold_vehicle_shooters_warn_on_mech_damage_without_blocking_it() {
    let tracked = include_str!("../game/mechs/Demolisher.toml");
    for source in [
        tracked.to_owned(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").to_owned(),
    ] {
        let (_dir, config, mut base, _, [target, _, shooter, _]) = engagement(&source).await;
        let high = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
            .unwrap();
        seed(&mut base, shooter, high);
        let mut ordinary = base.clone();
        let expected =
            fire_battle_vehicle_shot(&mut ordinary, shooter, ObjectId(1), target, 0, fire_rules())
                .unwrap();
        set_battle_weapons_hold(&mut base, shooter, true).unwrap();
        let report =
            fire_battle_vehicle_shot(&mut base, shooter, ObjectId(1), target, 0, fire_rules())
                .unwrap();
        assert!(report.launch.hit);
        let notices = report.notices();
        assert!(
            notices
                .iter()
                .any(|n| n.unit == shooter && n.text == "You are currently in weapons hold!")
        );
        assert_eq!(
            notices
                .into_iter()
                .filter(|n| n.text != "You are currently in weapons hold!")
                .collect::<Vec<_>>(),
            expected.notices()
        );
        set_battle_weapons_hold(&mut base, shooter, false).unwrap();
        assert_eq!(base.btech, ordinary.btech);
        base.validate(&config).unwrap();
    }
}

/// Both shooter anatomies retain attribution against every supported vehicle target, including safe routing.
#[tokio::test]
async fn weapons_hold_vehicle_targets_preserve_damage_safety_and_restart() {
    let tracked = include_str!("../game/mechs/Demolisher.toml");
    for source in [
        tracked.to_owned(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").to_owned(),
    ] {
        let (_dir, config, initial, map, ids) = engagement(&source).await;
        for vehicle_shooter in [false, true] {
            for safe in [false, true] {
                let mut base = initial.clone();
                let shooter = if vehicle_shooter { ids[2] } else { ids[0] };
                let target = ids[3];
                release_battle_pilot(&mut base, ids[2], ObjectId(1)).unwrap();
                power(&mut base, &[shooter], BattlePower::Off);
                place_battle_unit(&mut base, shooter, map, 0, 1).unwrap();
                power(&mut base, &[shooter], BattlePower::Running);
                base.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
                assign_battle_pilot(&mut base, shooter, ObjectId(1)).unwrap();
                refresh_battle_contacts(&mut base, &[shooter]).unwrap();
                let high = (0..=255)
                    .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
                    .unwrap();
                let mut saved = serde_json::to_value(&base.btech).unwrap();
                let collection = if vehicle_shooter {
                    "vehicles"
                } else {
                    "constructed"
                };
                saved[collection][shooter.0.to_string()]["dice"] =
                    serde_json::to_value(BattleDice::seeded([high; 32])).unwrap();
                base.btech = serde_json::from_value(saved).unwrap();
                let mut rules = fire_rules();
                rules.shot.vehicle_impact.criticals.combat_safe = safe;
                let fire = |world: &mut World| {
                    if vehicle_shooter {
                        let report =
                            fire_battle_vehicle_shot(world, shooter, ObjectId(1), target, 0, rules)
                                .unwrap();
                        assert!(report.launch.hit);
                        (report.notices(), serde_json::to_value(report).unwrap())
                    } else {
                        let report =
                            resolve_battle_shot(world, shooter, ObjectId(1), target, 0, rules.shot)
                                .unwrap();
                        assert!(report.salvo.is_some());
                        (report.notices(), serde_json::to_value(report).unwrap())
                    }
                };
                let mut ordinary = base.clone();
                let (expected, _) = fire(&mut ordinary);
                set_battle_weapons_hold(&mut base, shooter, true).unwrap();
                persistence::save(&config.database(), &base).await.unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let (notices, report) = fire(&mut base);
                let warnings: Vec<_> = notices
                    .iter()
                    .filter(|n| n.text == "You are currently in weapons hold!")
                    .collect();
                assert_eq!(warnings.is_empty(), safe);
                assert!(warnings.iter().all(|n| n.unit == shooter));
                assert_eq!(
                    notices
                        .into_iter()
                        .filter(|n| n.text != "You are currently in weapons hold!")
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(fire(&mut replay).1, report);
                assert_eq!(base.btech, replay.btech);
                set_battle_weapons_hold(&mut base, shooter, false).unwrap();
                assert_eq!(base.btech, ordinary.btech);
                base.validate(&config).unwrap();
            }
        }
    }
}

/// Armor penetration is one entry; ammunition explosions retain the shooter as a new entry.
#[tokio::test]
async fn weapons_hold_vehicle_critical_cascades_keep_incoming_attacker() {
    let (_dir, config, mut base, _, [_, _, shooter, target]) =
        engagement(include_str!("../game/mechs/Demolisher.toml")).await;
    let high = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut base, shooter, high);
    base.btech
        .rewrite_unit_record(target, |record| {
            for section in record["sections"].as_object_mut().unwrap().values_mut() {
                section["armor"] = 0.into();
            }
        })
        .unwrap();
    let mut rules = fire_rules();
    rules.shot.vehicle_impact.criticals.enabled = true;
    rules.shot.vehicle_impact.criticals.table = BattleVehicleCriticalTable::Advanced;
    let mut seen = [false; 2];
    for value in 0..=255 {
        let mut ordinary = base.clone();
        seed(&mut ordinary, target, value);
        let mut held = ordinary.clone();
        let expected =
            fire_battle_vehicle_shot(&mut ordinary, shooter, ObjectId(1), target, 0, rules)
                .unwrap();
        let Some(BattleTargetSalvo::Vehicle(salvo)) = &expected.salvo else {
            panic!("Shot must hit the vehicle")
        };
        let damage = salvo.groups[0].impact.damage.as_ref().unwrap();
        let explosion = damage
            .internal
            .iter()
            .flat_map(|internal| &internal.criticals)
            .chain(&damage.criticals)
            .any(|critical| !critical.internal_damage.is_empty());
        if !explosion && damage.internal.is_none() {
            continue;
        }
        if seen[usize::from(explosion)] {
            continue;
        }
        set_battle_weapons_hold(&mut held, shooter, true).unwrap();
        let report =
            fire_battle_vehicle_shot(&mut held, shooter, ObjectId(1), target, 0, rules).unwrap();
        let notices = report.notices();
        let warnings: Vec<_> = notices
            .iter()
            .filter(|n| n.text == "You are currently in weapons hold!")
            .collect();
        assert!(warnings.iter().all(|n| n.unit == shooter));
        if explosion {
            assert!(warnings.len() > 1);
        } else {
            assert_eq!(warnings.len(), 1);
        }
        assert_eq!(
            notices
                .into_iter()
                .filter(|n| n.text != "You are currently in weapons hold!")
                .collect::<Vec<_>>(),
            expected.notices()
        );
        set_battle_weapons_hold(&mut held, shooter, false).unwrap();
        assert_eq!(held.btech, ordinary.btech);
        held.validate(&config).unwrap();
        seen[usize::from(explosion)] = true;
        if seen == [true; 2] {
            break;
        }
    }
    assert_eq!(
        seen, [true; 2],
        "Exercise both penetration alone and an ammunition cascade"
    );
}

/// Host configuration reaches every shooter/target pairing through native and Lua firing.
#[tokio::test]
async fn configured_energy_range_damage_is_shared_by_all_unit_pairings() {
    let (dir, _config, base, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Savannah_Master.toml"),
    )
    .await;

    let attack_seed = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    for shooter in [ids[0], ids[2]] {
        for target in [ids[1], ids[3]] {
            for enabled in [false, true] {
                let path = dir.path().join("stompymux.toml");
                let mut source: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                let table = source
                    .as_table_mut()
                    .unwrap()
                    .entry("battletech")
                    .or_insert_with(|| toml::Value::Table(Default::default()))
                    .as_table_mut()
                    .unwrap();
                table.insert(
                    "moddamagewithrange".into(),
                    toml::Value::Integer(i64::from(enabled)),
                );
                table.insert("glancing_blows".into(), toml::Value::Integer(0));
                std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
                let config = Config::load(dir.path()).unwrap();
                let mut world = base.clone();
                place_battle_unit(&mut world, shooter, map, 0, 1).unwrap();
                power(&mut world, &ids, BattlePower::Running);
                world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
                assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
                refresh_battle_contacts(&mut world, &[shooter]).unwrap();
                world
                    .btech
                    .rewrite_unit_record(shooter, |record| {
                        record["dice"] =
                            serde_json::to_value(BattleDice::seeded([attack_seed; 32])).unwrap();
                    })
                    .unwrap();
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                    .unwrap();
                let damage: u16 = lua.eval_callback(&format!("local r=btech.unit.fire({},1,0,{}); return r.salvo.report.groups[1].damage",shooter.0,target.0)).unwrap();
                assert_eq!(
                    damage,
                    if enabled { 6 } else { 5 },
                    "shooter={shooter:?} target={target:?}"
                );
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
    }
}

/// Both shooter families preserve a struck aircraft's private emergency check through firing.
#[tokio::test]
async fn weapon_fire_preserves_target_emergency_feedback() {
    for vehicle_shooter in [false, true] {
        let (_dir, config, mut base, map, ids) =
            engagement(include_str!("../game/mechs/Demolisher.toml")).await;
        let shooter = ids[if vehicle_shooter { 2 } else { 0 }];
        release_battle_pilot(&mut base, ids[2], ObjectId(1)).unwrap();
        power(&mut base, &[shooter], BattlePower::Off);
        place_battle_unit(&mut base, shooter, map, 0, 1).unwrap();
        power(&mut base, &[shooter], BattlePower::Running);
        base.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
        assign_battle_pilot(&mut base, shooter, ObjectId(1)).unwrap();
        let target = base.create(&config, "Emergency target".into(), Kind::Thing);
        base.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
        let mut template =
            BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
                .unwrap();
        for section in template.sections.values_mut() {
            section.internal = 30;
        }
        create_battle_vehicle(&mut base, target, template).unwrap();
        place_battle_unit(&mut base, target, map, 0, 0).unwrap();
        base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
        base.objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        assign_battle_pilot(&mut base, target, ObjectId(2)).unwrap();
        let passenger = base.create(&config, "Passenger".into(), Kind::Player);
        base.objects.get_mut(&passenger).unwrap().location = Some(target);
        base.objects
            .get_mut(&passenger)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let high = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let mut state = serde_json::to_value(&base.btech).unwrap();
        state[if vehicle_shooter {
            "vehicles"
        } else {
            "constructed"
        }][shooter.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([high; 32])).unwrap();
        let aircraft = &mut state["vehicles"][target.0.to_string()];
        aircraft["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        aircraft["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Airborne,
            altitude: 2.0,
            vertical_speed: 0.0,
        })
        .unwrap();
        for section in aircraft["sections"].as_object_mut().unwrap().values_mut() {
            section["armor"] = 0.into();
            section["internal"] = 30.into();
        }
        base.btech = serde_json::from_value(state).unwrap();
        refresh_battle_contacts(&mut base, &[shooter]).unwrap();
        select_battle_target(&mut base, shooter, ObjectId(1), Some(target)).unwrap();
        let call = format!("btech.unit.fire({},1,0)", shooter.0);
        let mut covered = false;
        for value in 0..=255 {
            let mut world = base.clone();
            seed(&mut world, target, value);
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            scripts.eval_callback::<()>(&call).unwrap();
            let output = scripts.drain_outbox();
            if !output
                .iter()
                .any(|(_, message)| message.source() == "You make a piloting skill roll!")
            {
                continue;
            }
            let messages: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(2))
                .map(|(_, message)| message.source())
                .collect();
            let index = messages
                .iter()
                .position(|message| *message == "You make a piloting skill roll!")
                .unwrap();
            assert_eq!(messages[index - 1], "Your engine takes a direct hit!");
            assert!(messages[index + 1].starts_with("Modified Pilot Skill:"));
            assert!(
                messages[index + 2] == "You land safely!"
                    || messages[index + 2] == "You lose lift and start to fall!"
            );
            assert!(output.iter().any(|(who, _)| *who == passenger));
            assert!(!output.iter().any(|(who, message)| *who != ObjectId(2)
                && (message.source() == "You make a piloting skill roll!"
                    || message.source().starts_with("Modified Pilot Skill:"))));
            let replay = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            assert!(
                replay
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(replay.world().btech, world.btech);
            assert!(replay.drain_outbox().is_empty());
            replay.eval_callback::<()>(&call).unwrap();
            assert_eq!(scripts.world().btech, replay.world().btech);
            assert_eq!(output, replay.drain_outbox());
            covered = true;
            break;
        }
        assert!(
            covered,
            "shooter family {vehicle_shooter} must cause an emergency check"
        );
    }
}
