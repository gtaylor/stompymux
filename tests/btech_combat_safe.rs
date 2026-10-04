//! Scenario combat immunity protects shared damage paths without blocking weapon expenditure.
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
    world.btech.rewrite_unit_record(id, change).unwrap();
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
        BattleMapAsset::from_cells(if blocked {
            "1 5\n.0\n.9\n.0\n.0\n.0\n"
        } else {
            "1 5\n.0\n.0\n.0\n.0\n.0\n"
        })
        .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut ids = Vec::new();
    for (index, source) in [observer, target].into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 3 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
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

/// Material and crew protection is compared independently of target random-state advancement.
fn state(world: &World, id: ObjectId) -> serde_json::Value {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return serde_json::to_value(unit).unwrap();
    }
    serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()
}

/// Either launcher feeds its accepted hit to the same target immunity decision.
fn fire(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
) -> (Vec<BattleNotice>, serde_json::Value) {
    if world.btech.vehicles().contains_key(&shooter) {
        let report = fire_battle_vehicle_shot(
            world,
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
        assert!(report.launch.hit, "{report:?}");
        return (report.notices(), serde_json::to_value(report).unwrap());
    }
    let report = resolve_battle_shot(world, shooter, ObjectId(1), target, 0, shot_rules()).unwrap();
    assert!(report.salvo.is_some(), "{report:?}");
    (report.notices(), serde_json::to_value(report).unwrap())
}

/// All supported attacker/target pairs retain launch cost but suppress damage and hold warnings.
#[tokio::test]
async fn immunity_covers_all_chassis_pairs_and_replays() {
    for attacker in templates() {
        for target_source in templates() {
            let (_dir, config, mut initial, shooter, target) =
                fixture(&attacker, &target_source, false).await;
            let high = (0..=255)
                .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
                .unwrap();
            acquire(&mut initial, shooter, target);
            edit(&mut initial, shooter, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded([high; 32])).unwrap()
            });
            let position = if let Some(unit) = initial.btech.vehicles().get(&shooter) {
                unit.position()
            } else {
                initial.btech.constructed_units()[&shooter].position()
            }
            .unwrap();
            for case in ["unit", "map", "entrance", "attacker"] {
                let mut world = initial.clone();
                if case == "unit" {
                    set_battle_combat_safe(&mut world, target, true).unwrap();
                }
                if case == "attacker" {
                    set_battle_combat_safe(&mut world, shooter, true).unwrap();
                }
                if case == "map" || case == "entrance" {
                    let mut building = world.btech.maps()[&position.map].building;
                    building.flags = if case == "map" { 2 } else { 8 };
                    set_building_state(&mut world, position.map, building).unwrap();
                }
                set_battle_weapons_hold(&mut world, shooter, true).unwrap();
                let before = state(&world, target);
                let launcher = state(&world, shooter);
                persistence::save(&config.database(), &world).await.unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let (notices, report) = fire(&mut world, shooter, target);
                let after = state(&world, target);
                let safe = matches!(case, "unit" | "map");
                let paint: Vec<_> = notices
                    .iter()
                    .filter(|n| n.text == "Your efforts only scratch the paint!")
                    .collect();
                assert_eq!(!paint.is_empty(), safe, "{case}: {notices:?}");
                assert!(paint.iter().all(|n| n.unit == shooter));
                assert_eq!(
                    notices
                        .iter()
                        .any(|n| n.text == "You are currently in weapons hold!"),
                    !safe
                );
                if safe {
                    for field in [
                        "sections",
                        "lost_criticals",
                        "pilot_injuries",
                        "stagger",
                        "signature",
                    ] {
                        if case == "map"
                            && field == "sections"
                            && before[field].get("rotor").is_some()
                        {
                            // Map immunity begins after hit-table rotor consequences; hull armor remains protected.
                            for (section, state) in before[field].as_object().unwrap() {
                                if section != "rotor" {
                                    assert_eq!(state, &after[field][section]);
                                }
                            }
                        } else {
                            assert_eq!(before[field], after[field], "{case} {field}");
                        }
                    }
                } else {
                    assert_ne!(before["sections"], after["sections"]);
                }
                assert_ne!(
                    launcher,
                    state(&world, shooter),
                    "Launch still spends ammunition/recycle/heat"
                );
                assert_eq!(fire(&mut replay, shooter, target).1, report);
                assert_eq!(world.btech, replay.btech);
                world.validate(&config).unwrap();
            }
        }
    }
}

/// Administrator and trusted script state is durable, validates liveness, and rolls back on failure.
#[tokio::test]
async fn immunity_controls_are_atomic_across_chassis() {
    for source in templates() {
        let (_dir, config, mut world, id, _) = fixture(&source, &source, false).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let command = format!("@btech unit-combat-safe #{}=on", id.0);
        support::run_text(&scripts, &config, ObjectId(2), 1, &command);
        assert!(!battle_combat_safe(&scripts.world(), id).unwrap());
        support::run_text(&scripts, &config, ObjectId(1), 1, &command);
        assert!(battle_combat_safe(&scripts.world(), id).unwrap());
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.combat_safe({},false); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let enabled: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).combat_safe and btech.unit.combat_safe({})",
                id.0, id.0
            ))
            .unwrap();
        assert!(enabled);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(replay.btech, before);
        scripts
            .eval_callback::<()>(&format!("btech.unit.combat_safe({},false)", id.0))
            .unwrap();
        assert!(!battle_combat_safe(&scripts.world(), id).unwrap());
        scripts
            .world_mut()
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(set_battle_combat_safe(&mut scripts.world_mut(), id, true).is_err());
    }
}

/// Environmental damage uses its own map, while unit immunity also skips fall injury and packets.
#[tokio::test]
async fn immunity_protects_self_damage_and_fall_crew() {
    for source in templates() {
        let (_dir, config, initial, id, _) = fixture(&source, &source, false).await;
        for unit_safe in [false, true] {
            let mut world = initial.clone();
            set_battle_combat_safe(&mut world, id, unit_safe).unwrap();
            let position = if let Some(unit) = world.btech.vehicles().get(&id) {
                unit.position()
            } else {
                world.btech.constructed_units()[&id].position()
            }
            .unwrap();
            let mut building = world.btech.maps()[&position.map].building;
            building.flags = 2;
            set_building_state(&mut world, position.map, building).unwrap();
            let before = state(&world, id);
            let rules = BattleFallRules::configured(&config);
            let report = if world.btech.vehicles().contains_key(&id) {
                serde_json::to_value(resolve_battle_vehicle_fall(&mut world, id, 2, rules).unwrap())
                    .unwrap()
            } else {
                serde_json::to_value(resolve_battle_fall(&mut world, id, 2, rules).unwrap())
                    .unwrap()
            };
            assert_eq!(report["avoidance"].is_null(), unit_safe);
            assert_eq!(state(&world, id)["sections"], before["sections"]);
            if unit_safe {
                assert_eq!(
                    state(&world, id)["pilot_injuries"],
                    before["pilot_injuries"]
                );
                assert!(report["pilot_injury"].is_null());
            }
            assert!(!report.to_string().contains("scratch the paint"));
            world.validate(&config).unwrap();
        }
    }
}

/// Unit immunity avoids location side effects and random critical selection; map immunity acts later.
#[tokio::test]
async fn immunity_preserves_hit_routing_and_internal_damage_boundaries() {
    for source in templates() {
        let (_dir, config, mut world, id, _) = fixture(&source, &source, false).await;
        set_battle_combat_safe(&mut world, id, true).unwrap();
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = true.into()
        });
        let before = state(&world, id);
        if world.btech.vehicles().contains_key(&id) {
            let rules = BattleFallRules::configured(&config)
                .vehicle_impact
                .criticals;
            let selection =
                roll_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                    .unwrap();
            assert!(selection.rolls.is_empty() && selection.effect.is_none());
            assert_eq!(state(&world, id), before);
            let report = resolve_battle_vehicle_internal_damage(
                &mut world,
                id,
                BattleVehicleSection::Front,
                100,
                rules,
            )
            .unwrap();
            assert_eq!(report.absorbed, 0);
            assert_eq!(report.rolls.len(), 1);
            assert!(report.criticals.is_empty() && report.notices.is_empty());
        } else {
            let mut unit = world.btech.constructed_units()[&id].clone();
            assert!(unit.choose_critical(BattleSection::CenterTorso).is_none());
            assert_eq!(&unit, &world.btech.constructed_units()[&id]);
            let rules = BattleHitRules {
                inferno_penalty: false,
                exile_stun_mode: 1,
            };
            for roll in [2, 12] {
                let mut dice = BattleDice::seeded([19; 32]);
                let original = dice.clone();
                let hit = rules
                    .resolve(&unit, BattleHitArc::Front, roll, &mut dice)
                    .unwrap();
                assert_eq!(hit.section, BattleSection::LeftArm);
                assert!(!hit.crew_stun && !hit.through_armor_critical);
                assert_eq!(dice, original);
            }
            let report = resolve_battle_impact(
                &mut world,
                id,
                BattleHit {
                    section: BattleSection::Head,
                    rear_armor: false,
                    through_armor_critical: true,
                    crew_stun: true,
                },
                100,
            )
            .unwrap();
            assert!(
                report.phases.is_empty()
                    && report.criticals.is_empty()
                    && report.pending_effects.is_empty()
            );
        }
        let after = state(&world, id);
        assert_eq!(before["sections"], after["sections"]);
        assert_eq!(before["signature"], after["signature"]);
        assert_ne!(before["dice"], after["dice"]);
        world.validate(&config).unwrap();
    }
}

/// Successful physical attacks retain limb recovery and feedback while immunity protects the victim.
#[tokio::test]
async fn immunity_protects_physical_damage_for_bipeds_and_quads() {
    for source in templates().into_iter().take(2) {
        let (_dir, config, mut world, attacker, target) = fixture(&source, &source, false).await;
        let map = world.btech.constructed_units()[&attacker]
            .position()
            .unwrap()
            .map;
        edit(&mut world, attacker, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut world, attacker, map, 0, 0).unwrap();
        edit(&mut world, attacker, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        acquire(&mut world, attacker, target);
        let high = (0..=255)
            .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
            .unwrap();
        edit(&mut world, attacker, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([high; 32])).unwrap()
        });
        set_battle_combat_safe(&mut world, target, true).unwrap();
        set_battle_weapons_hold(&mut world, attacker, true).unwrap();
        let before = state(&world, target);
        let report = resolve_battle_kick(
            &mut world,
            attacker,
            ObjectId(1),
            target,
            BattleLeg::Left,
            BattlePhysicalRules {
                use_pilot_skill: false,
                fasa_turning: false,
                extended_movement: false,
                hit_arc_mode: 0,
                glancing: BattleGlancingMode::Disabled,
                fall: BattleFallRules::configured(&config),
            },
        )
        .unwrap();
        assert!(report.hit);
        let impact = report.impact.unwrap();
        assert!(impact.impact.phases.is_empty());
        assert!(
            impact
                .notices
                .iter()
                .any(|n| n.unit == attacker && n.text == "Your efforts only scratch the paint!")
        );
        assert!(
            !impact
                .notices
                .iter()
                .any(|n| n.text.contains("weapons hold"))
        );
        assert_eq!(before["sections"], state(&world, target)["sections"]);
        world.validate(&config).unwrap();
    }
}

/// A breached submerged compartment is protected by the unit flag, independently of map safety.
#[tokio::test]
async fn immunity_blocks_flooding_until_the_unit_flag_is_removed() {
    for source in templates().into_iter().take(2) {
        let (_dir, config, mut world, id, _) = fixture(&source, &source, false).await;
        let map = world.create(&config, "Water".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "water",
            BattleMapAsset::from_cells("1 1\n~2\n").unwrap(),
        )
        .unwrap();
        edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            unit["sections"]["LeftLeg"]["armor"] = 0.into();
        });
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        set_battle_combat_safe(&mut world, id, true).unwrap();
        let before = world.btech.clone();
        let rules = BattleFallRules::configured(&config);
        assert!(flood_battle_unit(&mut world, id, rules).unwrap().is_empty());
        assert_eq!(world.btech, before);
        set_battle_combat_safe(&mut world, id, false).unwrap();
        let mut building = world.btech.maps()[&map].building;
        building.flags = 2;
        set_building_state(&mut world, map, building).unwrap();
        let reports = flood_battle_unit(&mut world, id, rules).unwrap();
        assert!(
            reports
                .iter()
                .any(|report| report.section == BattleSection::LeftLeg)
        );
        world.validate(&config).unwrap();
    }
}

/// Acquire the perceived target before attack dice are set; ordinary acquisition rolls no dice.
fn acquire(world: &mut World, shooter: ObjectId, target: ObjectId) {
    refresh_battle_contacts(world, &[shooter]).unwrap();
    assert!(
        visible_battle_contact(world, shooter, target)
            .unwrap()
            .is_some(),
        "Fixture target could not be acquired"
    );
}
