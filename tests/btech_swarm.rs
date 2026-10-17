//! Swarm supplies and flights reuse controls, launch transactions and damage across chassis.
use crate::support;
use crate::support::btech_defense as defense_support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Representative construction for every supported movement class.
fn templates() -> Vec<String> {
    let tracked = include_str!("../game/units/Demolisher.toml");
    vec![
        include_str!("../game/units/JR7-D.toml").into(),
        include_str!("../game/units/GOL-1H.toml").into(),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        crate::support::templates::hovercraft(tracked),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 3", "walk_mp = 0"),
        include_str!("../game/units/Kestrel.toml").into(),
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
        MapAsset::from_cells(if blocked {
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
        (if index == 0 {
            crate::support::templates::fitted_unit(launcher(source, "Swarm"))
        } else {
            crate::support::templates::fitted_unit(UnitTemplate::parse("test", source).unwrap())
        })
        .create(&mut world, id)
        .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 3 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(Power::Running).unwrap();
            state["dice"] = serde_json::to_value(Dice::seeded([19; 32])).unwrap();
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
fn rules() -> AimRules {
    AimRules {
        override_weapon_arcs: false,
    }
}

/// Tactical conventional shot configuration shared by admission cases.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: stompymux_rs::VehicleImpactRules::STANDARD,
        aim: rules(),
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// Install the same Inner Sphere LRM-20 and full Swarm and Swarm-I bins in either construction
/// anatomy; Swarm-I is Inner Sphere technology (Tactical Operations: AUE p. 183).
fn launcher(source: &str, mode: &str) -> UnitTemplate {
    let mut definition =
        crate::support::templates::fitted_unit(UnitTemplate::parse("test", source).unwrap());
    let part = MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
        .unwrap()
        .sections[&MechSection::LeftArm]
        .criticals[&2]
        .clone();
    let install = |section: &mut SectionDefinition, mech: bool| {
        let mut mount = part.clone();
        mount.equipment = Weapon::Lrm20.name().into();
        mount.modes = vec![mode.into()];
        mount.data = "-".into();
        for slot in 0..if mech { 5 } else { 1 } {
            section.criticals.insert(slot, mount.clone());
        }
        let mut bin = mount;
        bin.equipment = format!("Ammo_{}", Weapon::Lrm20.name());
        bin.data = "6".into();
        section.criticals.insert(6, bin.clone());
        bin.modes = vec![if mode == "Swarm" { "Swarm1" } else { "Swarm" }.into()];
        section.criticals.insert(7, bin);
    };
    match &mut definition {
        UnitTemplate::Mech(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, part| {
                    !part.equipment.starts_with("Ammo_")
                        && !Weapon::ALL.iter().any(|w| w.name() == part.equipment)
                });
            }
            install(
                unit.sections.get_mut(&MechSection::RightTorso).unwrap(),
                true,
            );
        }
        UnitTemplate::Vehicle(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, part| {
                    !part.equipment.starts_with("Ammo_")
                        && !Weapon::ALL.iter().any(|w| w.name() == part.equipment)
                });
            }
            install(
                unit.sections.get_mut(&VehicleSection::Front).unwrap(),
                false,
            );
        }
    }
    crate::support::templates::legal_unit(definition)
}

/// Seed an attack independently from cluster and target damage streams.
fn dice(world: &mut World, id: ObjectId, total: u8) {
    let seed = (0..=255)
        .find(|v| Dice::seeded([*v; 32]).two_d6() == total)
        .unwrap();
    edit(world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
    });
}

/// Preserve known contacts without spending launch dice.
fn acquire(world: &mut World, shooter: ObjectId, target: ObjectId) {
    for seed in 0..=255 {
        edit(world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
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

/// Normalize only the report envelope; firing still uses each public launcher API.
fn fire(world: &mut World, shooter: ObjectId, target: ObjectId) -> Option<SwarmReport> {
    fire_with_rules(world, shooter, target, shot_rules())
}

/// Keep launch normalization identical while testing configured target-resolution boundaries.
fn fire_with_rules(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    rules: ShotRules,
) -> Option<SwarmReport> {
    let report = fire_battle_unit_shot(
        world,
        shooter,
        ObjectId(1),
        target,
        0,
        VehicleShotRules {
            shot: rules,
            shooter_criticals: VehicleImpactRules::STANDARD.criticals,
        },
    )
    .unwrap();
    let salvo = stompymux_rs::by_chassis!(report, |report| report.salvo);
    salvo.map(|salvo| {
        let TargetSalvo::Swarm(report) = salvo else {
            panic!("Swarm report missing")
        };
        report
    })
}

/// Construct real launchers on all seven chassis and replay the same missile flight; with no
/// other unit near the target, the missiles that miss it fly no farther.
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
            gunnery(&mut miss, 8);
            dice(&mut miss, shooter, 2);
            let missed = fire(&mut miss, shooter, target).unwrap();
            assert_eq!(missed.hops.len(), 1);
            assert!(missed.hops[0].salvo.is_none());
            assert_eq!(missed.remaining, 20);
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
    crate::support::templates::fitted_unit(UnitTemplate::parse("test", source).unwrap())
        .create(world, id)
        .unwrap();
    support::seed_object_dice(world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, id, map, 0, 0).unwrap();
    edit(world, id, |unit| {
        unit["signature"]["team"] = team.into();
    });
    dice(world, id, 12);
    id
}

/// Move a unit administratively to another hex of its battlefield.
fn relocate(world: &mut World, id: ObjectId, y: i64) {
    let map = if let Some(unit) = world.btech.vehicles().get(&id) {
        unit.position().unwrap().map
    } else {
        world.btech.constructed_units()[&id].position().unwrap().map
    };
    let power = if let Some(unit) = world.btech.vehicles().get(&id) {
        serde_json::to_value(unit).unwrap()["power"].clone()
    } else {
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["power"].clone()
    };
    edit(world, id, |unit| {
        unit["power"] = serde_json::to_value(Power::Off).unwrap()
    });
    place_battle_unit(world, id, map, 0, y).unwrap();
    edit(world, id, |unit| unit["power"] = power);
}

/// The missiles that miss a target fly on to the nearest unit, friend or foe, in its hex
/// before the adjacent ones, needing no line of sight and skipping combat-safe units; the
/// later attack rolls on the Cluster Hits column for the missiles still flying, and Swarm-I
/// takes +2 against a friend (Tactical Operations: AUE p. 183).
#[tokio::test]
async fn swarm_retarget_selection_follows_distance_and_identifies_friends() {
    for source in templates() {
        for friend_or_foe in [false, true] {
            let (_dir, config, mut initial, shooter, target) =
                fixture(&source, &templates()[0], false).await;
            let safe = candidate(&mut initial, &config, target, &templates()[1], 1);
            set_battle_combat_safe(&mut initial, safe, true).unwrap();
            let friendly = candidate(&mut initial, &config, target, &templates()[2], 0);
            let hostile = candidate(&mut initial, &config, target, &templates()[6], 1);
            relocate(&mut initial, hostile, 1);
            acquire(&mut initial, shooter, target);
            if friend_or_foe {
                assert_eq!(
                    toggle_battle_swarm(&mut initial, shooter, ObjectId(1), 0, true).unwrap(),
                    AmmunitionMode::Swarm1
                );
            }
            dice(&mut initial, target, 2);
            // The first secondary attack hits with a 12 so the column caps the hits.
            let seed = (0u32..65536)
                .find_map(|n| {
                    let mut seed = [0; 32];
                    seed[..4].copy_from_slice(&n.to_le_bytes());
                    let mut dice = Dice::seeded(seed);
                    (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
                })
                .unwrap();
            edit(&mut initial, shooter, |unit| {
                unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
            });
            let report = fire(&mut initial, shooter, target).unwrap();
            assert_eq!(report.hops.len(), 2);
            assert_eq!(report.hops[1].target, friendly);
            assert_eq!(report.hops[1].incoming, 14);
            assert_eq!(report.remaining, 0);
            let TargetSalvo::Vehicle(salvo) = report.hops[1].salvo.as_ref().unwrap() else {
                panic!("Expected vehicle target")
            };
            assert_eq!(salvo.missiles_before_defense, Some(14));
            // The hostile unit next door is never reached: the friend in the target's hex is
            // nearer, and the missiles are spent on it.
            assert!(report.hops.iter().all(|hop| hop.target != hostile));
            initial.validate(&config).unwrap();
        }
    }
}

/// Swarm-I takes +2 against a friendly unit that Swarm attacks at its ordinary target number.
#[tokio::test]
async fn swarm1_takes_two_against_friends() {
    let mut numbers = Vec::new();
    for friend_or_foe in [false, true] {
        let (_dir, config, mut world, shooter, target) =
            fixture(&templates()[0], &templates()[0], false).await;
        let friendly = candidate(&mut world, &config, target, &templates()[2], 0);
        acquire(&mut world, shooter, target);
        if friend_or_foe {
            toggle_battle_swarm(&mut world, shooter, ObjectId(1), 0, true).unwrap();
        }
        dice(&mut world, target, 2);
        dice(&mut world, shooter, 12);
        let report = fire(&mut world, shooter, target).unwrap();
        assert_eq!(report.hops[1].target, friendly);
        numbers.push(report.hops[1].target_number);
    }
    assert_eq!(numbers[1], numbers[0] + 2);
}

/// Secondary misses keep their incoming missiles and proceed to the next nearest unit.
#[tokio::test]
async fn swarm_secondary_misses_preserve_missiles() {
    let (_dir, config, mut world, shooter, target) =
        fixture(&templates()[0], &templates()[2], false).await;
    let second = candidate(&mut world, &config, target, &templates()[1], 1);
    let third = candidate(&mut world, &config, target, &templates()[2], 1);
    relocate(&mut world, third, 1);
    for id in [target, second] {
        set_battle_visibility(
            &mut world,
            id,
            Visibility {
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
            let mut dice = Dice::seeded(seed);
            (dice.two_d6() == 12 && dice.two_d6() == 2).then_some(seed)
        })
        .unwrap();
    edit(&mut world, shooter, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
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
            Visibility {
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
                let mut dice = Dice::seeded(seed);
                (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
            })
            .unwrap();
        edit(&mut world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
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
        Character {
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
    support::seed_object_dice(world, ObjectId(1), support::FIXTURE_DICE_SEED);
}

/// No unit is attacked twice by one flight; it ends when no unvisited unit is near the last
/// one attacked, however many it has attacked (Tactical Operations: AUE p. 183).
#[tokio::test]
async fn swarm_attacks_each_nearby_unit_once() {
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
        acquire(&mut world, shooter, target);
        // Every secondary attack misses, so the flight keeps all its missiles.
        gunnery(&mut world, 12);
        dice(&mut world, shooter, 12);
        dice(&mut world, target, 2);
        let report = fire(&mut world, shooter, target).unwrap();
        let mut attacked: Vec<_> = report.hops.iter().map(|hop| hop.target).collect();
        assert_eq!(attacked[0], target);
        attacked.sort();
        attacked.dedup();
        let mut expected = targets.clone();
        expected.sort();
        assert_eq!(attacked, expected);
        assert_eq!(report.hops.len(), 13);
    }
}

/// A flight is not limited by range or sight: it follows a line of units past the launcher's
/// long range.
#[tokio::test]
async fn swarm_follows_units_beyond_long_range() {
    let (_dir, config, mut world, shooter, target) =
        fixture(&templates()[0], &templates()[0], false).await;
    let map = world.create(&config, "Long swarm field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "long-flight",
        MapAsset::from_cells(&format!("1 32\n{}", ".0\n".repeat(32))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    for (id, y) in [(shooter, 21), (target, 0)] {
        edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(Power::Off).unwrap()
        });
        place_battle_unit(&mut world, id, map, 0, y).unwrap();
        edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(Power::Running).unwrap()
        });
    }
    let second = candidate(&mut world, &config, target, &templates()[2], 1);
    let third = candidate(&mut world, &config, target, &templates()[1], 1);
    place_battle_unit(&mut world, second, map, 0, 1).unwrap();
    place_battle_unit(&mut world, third, map, 0, 2).unwrap();
    acquire(&mut world, shooter, target);
    gunnery(&mut world, 12);
    dice(&mut world, shooter, 12);
    dice(&mut world, target, 2);
    let report = fire(&mut world, shooter, target).unwrap();
    assert_eq!(
        report.hops.iter().map(|hop| hop.target).collect::<Vec<_>>(),
        vec![target, second, third]
    );
}

/// A flight can turn on its own launcher; Swarm-1 attacks it too, two harder.
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
                unit["power"] = serde_json::to_value(Power::Off).unwrap()
            });
            place_battle_unit(&mut world, target, map, 0, 2).unwrap();
            set_battle_visibility(
                &mut world,
                target,
                Visibility {
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
            assert_eq!(report.hops.len(), 2);
            assert_eq!(report.hops[1].target, shooter);
        }
    }
}

/// Swarm's interception exemption preserves a real enabled defense's ammunition, heat and recycle.
#[tokio::test]
async fn swarm_skips_installed_ams_on_hits_and_misses() {
    for source in [templates()[0].clone(), templates()[2].clone()] {
        for target_source in [
            include_str!("../game/units/Daishi-A.toml"),
            include_str!("../game/units/Goblin-58.toml"),
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
                // Vehicles track no heat (Total Warfare p. 158), so only Mechs carry a heat field.
                let fields: &[&str] = if world.btech.vehicles().contains_key(&target) {
                    &["ammunition", "weapon_recycle"]
                } else {
                    &["ammunition", "heat", "weapon_recycle"]
                };
                for field in fields {
                    assert!(!before[*field].is_null(), "Missing defense field {field}");
                    assert_eq!(before[*field], after[*field], "{field}");
                }
            }
        }
    }
}

/// Swarm ammunition follows the same indirect-launcher eligibility, capacity and saved mode grammar.
#[test]
fn swarm_template_modes_cover_compatible_catalogue() {
    for &weapon in Weapon::ALL {
        for (flag, mode) in [
            ("Swarm", AmmunitionMode::Swarm),
            ("Swarm1", AmmunitionMode::Swarm1),
        ] {
            // Swarm-I is Inner Sphere technology (Tactical Operations: AUE p. 183).
            let compatible = if mode == AmmunitionMode::Swarm {
                weapon.is_standard_lrm()
            } else {
                weapon.supports_semiguided()
            };
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
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            let arm = template.sections.get_mut(&MechSection::LeftArm).unwrap();
            let mut mount = arm.criticals[&2].clone();
            mount.equipment = weapon.name().into();
            mount.modes = vec![flag.into()];
            for slot in 2..2 + weapon.profile().critical_slots {
                arm.criticals.insert(slot, mount.clone());
            }
            let bin = template
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            bin.modes = vec![flag.into()];
            let unit = Mech::from_template(crate::support::templates::legal(template)).unwrap();
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
            let restored: Mech =
                serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(restored, unit);
        }
    }
}

/// Dice seeds for the swarm attack and target preparation; pure rolls, shared across shards.
fn aimed_swarm_seeds() -> ([u8; 32], [u8; 32]) {
    static SEEDS: std::sync::OnceLock<([u8; 32], [u8; 32])> = std::sync::OnceLock::new();
    *SEEDS.get_or_init(|| {
        let seed_for = |predicate: &dyn Fn(&mut Dice) -> bool| {
            (0u32..65536)
                .find_map(|n| {
                    let mut seed = [0; 32];
                    seed[..4].copy_from_slice(&n.to_le_bytes());
                    predicate(&mut Dice::seeded(seed)).then_some(seed)
                })
                .unwrap()
        };
        (
            seed_for(&|dice| dice.two_d6() == 12 && dice.two_d6() >= 8 && dice.two_d6() >= 8),
            seed_for(&|dice| (6..=8).contains(&dice.two_d6()) && dice.two_d6() == 2),
        )
    })
}

/// Aimed Swarm flights prepare only their first target and bypass active defenses on every hop.
async fn aimed_swarm_matrix(source: &str) {
    let (attack, preparation) = aimed_swarm_seeds();
    let defenders = defense_support::templates();
    let mut probed_fidelity = [false; 2];
    for (index, recipient) in defenders.iter().enumerate() {
        let (_dir, config, mut base, shooter, target) = fixture(source, recipient, false).await;
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
        relocate(&mut base, third, 1);
        for id in [target, second, third] {
            edit(&mut base, id, |state| {
                state["power"] = serde_json::to_value(Power::Running).unwrap();
                state["fortified"] = true.into();
                state["ams_enabled"] = true.into();
                state["motion"]["heading"] = 180.0.into();
                state["motion"]["desired_heading"] = 180.0.into();
            });
            set_battle_visibility(
                &mut base,
                id,
                Visibility {
                    clairvoyant: true,
                    invisible: false,
                },
            )
            .unwrap();
        }
        acquire(&mut base, shooter, target);
        select_battle_target(&mut base, shooter, ObjectId(1), Some(target)).unwrap();
        for (shape, smart) in [false, true].into_iter().enumerate() {
            let mut world = base.clone();
            if smart {
                toggle_battle_swarm(&mut world, shooter, ObjectId(1), 0, true).unwrap();
            }
            edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded(attack)).unwrap()
            });
            edit(&mut world, target, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded(preparation)).unwrap()
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
            let rules = shot_rules();
            let report = fire_with_rules(&mut world, shooter, target, rules).unwrap();
            let baseline = fire_with_rules(&mut ordinary, shooter, target, rules).unwrap();
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
            // Swarm rounds are not exempt from AMS (Tactical Operations: AUE p. 183): the
            // initial target engages, while secondary hops are not engaged.
            for id in [target, second, third] {
                let recycle = if let Some(unit) = world.btech.vehicles().get(&id) {
                    unit.weapon_recycle()
                } else {
                    world.btech.constructed_units()[&id].weapon_recycle()
                };
                assert_eq!(recycle.is_empty(), id != target);
            }
            // Fidelity probe once per flight shape per shard.
            if !probed_fidelity[shape] {
                probed_fidelity[shape] = true;
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

/// One shard per shooter chassis; templates are listed in `templates`.
#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_01() {
    aimed_swarm_matrix(&templates()[0]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_02() {
    aimed_swarm_matrix(&templates()[1]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_03() {
    aimed_swarm_matrix(&templates()[2]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_04() {
    aimed_swarm_matrix(&templates()[3]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_05() {
    aimed_swarm_matrix(&templates()[4]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_06() {
    aimed_swarm_matrix(&templates()[5]).await;
}

#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets_07() {
    aimed_swarm_matrix(&templates()[6]).await;
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
        support::seed_object_dice(&mut base, ObjectId(2), support::FIXTURE_DICE_SEED);
        edit(&mut base, secondary, |unit| {
            unit["power"] = serde_json::to_value(Power::Running).unwrap()
        });
        for section in MechSection::ALL {
            let armor = base.btech.constructed_units()[&secondary].sections()[&section].armor;
            apply_damage_phase(
                &mut base,
                secondary,
                section,
                armor,
                DamagePhase::Armor { rear: false },
            )
            .unwrap();
        }
        set_battle_visibility(
            &mut base,
            target,
            Visibility {
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
                let mut dice = Dice::seeded(seed);
                (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
            })
            .unwrap();
        edit(&mut base, shooter, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded(launch_seed)).unwrap()
        });
        dice(&mut base, target, 2);
        let mut exercised = false;
        for seed in 0..=255 {
            let mut before = base.clone();
            edit(&mut before, secondary, |unit| {
                unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
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
