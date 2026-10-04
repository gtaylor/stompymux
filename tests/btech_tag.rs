//! Cross-chassis TAG ownership, equipment admission, native presentation and durable timers.
use crate::support;
use crate::support::btech_firing as firing;
use firing::{edit, fixture_with_target, templates};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Install TAG in a spare section slot through the ordinary template validation path.
fn install(world: &mut World, id: ObjectId, computer: bool) {
    let vehicle = world.btech.vehicles().contains_key(&id);
    edit(world, id, |state| {
        let section = if vehicle { "front" } else { "LeftTorso" };
        let count = if computer && !vehicle { 5 } else { 1 };
        for slot in 7..7 + count {
            state["definition"]["sections"][section]["criticals"][slot.to_string()] =
                serde_json::to_value(CriticalDefinition {
                    equipment: if computer { "C3Master" } else { "TAG" }.into(),
                    data: "-".into(),
                    modes: vec![],
                })
                .unwrap();
        }
    });
}

/// Uniform state inspection deliberately reads the public chassis adapters.
fn tag(world: &World, id: ObjectId) -> BattleTagState {
    world
        .btech
        .vehicles()
        .get(&id)
        .map(BattleVehicle::tag)
        .unwrap_or_else(|| world.btech.constructed_units()[&id].tag())
}

/// The equipment and target team changes precede contact acquisition.
async fn fixture(
    source: &str,
    target: &str,
    computer: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world, id, target, _) = fixture_with_target(source, None, target).await;
    install(&mut world, id, computer);
    edit(&mut world, target, |state| {
        state["signature"]["team"] = 2.into()
    });
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, id, target)
}

/// Every supported source/target family shares illumination, the exact timer and saved replay.
#[tokio::test]
async fn tag_across_chassis_native_lua_rollback_and_restart() {
    let sources = templates();
    for source in &sources {
        for target_source in [&sources[0], &sources[2], &sources[6]] {
            let (_dir, config, world, id, target) = fixture(source, target_source, false).await;
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let expression = format!("btech.unit.tag({},1,{})", id.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{expression}; error('abort TAG')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
            scripts.eval_callback::<()>(&expression).unwrap();
            let expected = scripts.world().btech.clone();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let label = world
                .btech
                .vehicles()
                .get(&target)
                .and_then(BattleVehicle::battlefield_id)
                .or_else(|| {
                    world
                        .btech
                        .constructed_units()
                        .get(&target)
                        .and_then(BattleUnit::battlefield_id)
                })
                .unwrap();
            let reply =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("tag {label}"));
            assert!(reply.contains("with your TAG."), "{reply}");
            assert_eq!(native.world().btech, expected);
            let mut active = native.world().clone();
            assert_eq!(battle_tagged_by(&active, target), Some(id));
            assert!(
                battle_unit_status(&active, id, "w")
                    .unwrap()
                    .contains("TAG([bold]")
            );
            for _ in 0..29 {
                assert!(advance_battle_tags(&mut active).is_empty());
            }
            persistence::save(&config.database(), &active)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, active.btech);
            assert!(
                advance_battle_tags(&mut restored)[0]
                    .text
                    .contains("stable lock")
            );
            assert_eq!(tag(&restored, id).remaining, 0);
            select_battle_tag(&mut restored, id, ObjectId(1), None).unwrap();
            for _ in 0..29 {
                assert!(advance_battle_tags(&mut restored).is_empty());
            }
            assert!(
                advance_battle_tags(&mut restored)[0]
                    .text
                    .contains("finished recycling")
            );
            assert_eq!(tag(&restored, id), BattleTagState::default());
            let key = if restored.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            assert_eq!(
                serde_json::to_value(&restored.btech).unwrap()[key][id.0.to_string()]["dice"],
                serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["dice"]
            );
            restored.validate(&config).unwrap();
        }
    }
}

/// Equipment failures take precedence over native syntax, and C3 masters provide integrated TAG.
#[tokio::test]
async fn tag_equipment_and_damage_have_distinct_replies() {
    for source in templates() {
        let (_dir, config, mut world, id, target, _) =
            fixture_with_target(&source, None, &templates()[0]).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let reply = support::run_text(&scripts, &config, ObjectId(1), 1, "tag");
        assert_eq!(
            text::plain(&reply).trim(),
            "This unit is not equipped with TAG!"
        );
        assert_eq!(scripts.world().btech, world.btech);
        install(&mut world, id, true);
        edit(&mut world, target, |state| {
            state["signature"]["team"] = 2.into()
        });
        select_battle_tag(&mut world, id, ObjectId(1), Some(target)).unwrap();
        assert_eq!(battle_tagged_by(&world, target), Some(id));
        if world.btech.vehicles().contains_key(&id) {
            destroy_battle_vehicle_critical(
                &mut world,
                id,
                VehicleCriticalLocation {
                    section: BattleVehicleSection::Front,
                    slot: 7,
                },
            )
            .unwrap();
        } else {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::LeftTorso,
                    slot: 7,
                },
            )
            .unwrap();
        }
        assert_eq!(battle_tagged_by(&world, target), None);
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let reply = support::run_text(&scripts, &config, ObjectId(1), 1, "tag nonsense extra");
        assert_eq!(text::plain(&reply).trim(), "Your TAG system is destroyed!");
        assert_eq!(scripts.world().btech, before);
        assert!(
            battle_unit_status(&scripts.world(), id, "w")
                .unwrap()
                .contains("TAG([fg=red bold]XX[reset])")
        );
    }
}

/// Administrative placement requires powering down vehicles; restore power without ticking sensors.
fn relocate(world: &mut World, id: ObjectId, map: ObjectId, y: i64) {
    edit(world, id, |state| {
        state["target_lock"] = serde_json::Value::Null;
        state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
    });
    place_battle_unit(world, id, map, 0, y).unwrap();
    edit(world, id, |state| {
        state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
    });
}

/// Range errors differ from lost contact and use the unrounded fifteen-hex boundary.
#[tokio::test]
async fn tag_range_visibility_and_syntax_fail_without_mutation() {
    for source in [&templates()[0], &templates()[2]] {
        let (_dir, config, mut world, id, target) = fixture(source, &templates()[2], false).await;
        let map = world.create(&config, "Long TAG lane".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "lane",
            BattleMapAsset::from_cells(&format!("1 20\n{}", ".0\n".repeat(20))).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        relocate(&mut world, id, map, 17);
        relocate(&mut world, target, map, 1);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        // Isolate the TAG range gate from the observer's perception reach.
        edit(&mut world, id, |state| {
            state["visibility"]["clairvoyant"] = true.into()
        });
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        assert!(battle_unit_range(&world, id, target).unwrap().spatial > 15.0);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for (command, expected) in [
            (
                format!("tag #{}", target.0),
                "Out of range! TAG ranges are 5/10/15",
            ),
            (
                "tag ZZ".into(),
                "That is not a valid TAG targetID. Try again.",
            ),
            ("tag".into(), "Invalid number of arguments to function!"),
            (
                "tag AA BB".into(),
                "Invalid number of arguments to function!",
            ),
        ] {
            let reply = support::run_text(&scripts, &config, ObjectId(1), 1, &command);
            assert_eq!(text::plain(&reply).trim(), expected);
            assert_eq!(scripts.world().btech, world.btech);
        }
        relocate(&mut world, target, map, 2);
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        assert_eq!(battle_unit_range(&world, id, target).unwrap().spatial, 15.0);
        let mut beyond = world.clone();
        edit(&mut beyond, target, |state| {
            let y = state["motion"]["point"]["y"].as_f64().unwrap();
            state["motion"]["point"]["y"] = (y - 0.01).into();
        });
        let before = beyond.btech.clone();
        assert_eq!(
            select_battle_tag(&mut beyond, id, ObjectId(1), Some(target))
                .unwrap_err()
                .to_string(),
            "Out of range! TAG ranges are 5/10/15"
        );
        assert_eq!(beyond.btech, before);
        select_battle_tag(&mut world, id, ObjectId(1), Some(target)).unwrap();
        assert_eq!(battle_tagged_by(&world, target), Some(id));
    }
}

/// A vehicle can replace a Mech illuminator and vice versa without duplicate target ownership.
#[tokio::test]
async fn tag_takeover_crosses_chassis_and_rejects_corrupt_saved_ownership() {
    let sources = templates();
    for (source, replacement) in [(&sources[0], &sources[2]), (&sources[2], &sources[0])] {
        let (_dir, config, mut world, id, target) = fixture(source, &sources[2], false).await;
        select_battle_tag(&mut world, id, ObjectId(1), Some(target)).unwrap();
        let map = world.btech.units()[&id].map.unwrap();
        let other = world.create(&config, "Replacement TAG".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", replacement)
            .unwrap()
            .create(&mut world, other)
            .unwrap();
        place_battle_unit(&mut world, other, map, 0, 11).unwrap();
        install(&mut world, other, false);
        edit(&mut world, other, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(other);
        assign_battle_pilot(&mut world, other, ObjectId(1)).unwrap();
        refresh_battle_contacts(&mut world, &[other]).unwrap();
        let notices = select_battle_tag(&mut world, other, ObjectId(1), Some(target)).unwrap();
        assert_eq!(notices.len(), 2);
        assert_eq!(
            tag(&world, id),
            BattleTagState {
                target: None,
                remaining: 30
            }
        );
        assert_eq!(battle_tagged_by(&world, target), Some(other));
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        edit(&mut world, id, |state| {
            state["tag"] = serde_json::json!({"target": target.0, "remaining":0})
        });
        assert!(
            world
                .validate(&config)
                .unwrap_err()
                .to_string()
                .contains("TAG ownership")
        );
    }
}

/// Vehicle shutdown publishes link loss immediately and recycling survives without an active engine.
#[tokio::test]
async fn vehicle_tag_shutdown_recycles_and_inspection_exposes_state() {
    for source in templates().iter().skip(2) {
        let (_dir, config, mut world, id, target) = fixture(source, &templates()[0], false).await;
        select_battle_tag(&mut world, id, ObjectId(1), Some(target)).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let selected: i64 = scripts
            .eval_callback(&format!("return btech.unit.state({}).tag.target", id.0))
            .unwrap();
        assert_eq!(selected, target.0);
        assert_eq!(scripts.world().btech, world.btech);
        let notices = stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleFallRules::configured(&config),
        )
        .unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "Your TAG connection has been broken.")
        );
        assert_eq!(
            tag(&world, id),
            BattleTagState {
                target: None,
                remaining: 30
            }
        );
        assert_eq!(battle_tagged_by(&world, target), None);
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
}

/// Vehicle illumination feeds the existing semi-guided aiming rule for either launcher chassis.
#[tokio::test]
async fn vehicle_tag_guides_mech_and_vehicle_missiles() {
    let sources = templates();
    for source in [&sources[0], &sources[2]] {
        let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
            source,
            Some(BattleWeapon::Lrm5),
            &sources[2],
            false,
            Some("Sguided"),
        )
        .await;
        edit(&mut world, target, |state| {
            state["signature"]["team"] = 2.into();
            state["motion"]["speed"] = 43.0.into();
            state["motion"]["desired_speed"] = 43.0.into();
        });
        edit(&mut world, shooter, |state| {
            state["ammunition_modes"][index.to_string()] =
                serde_json::to_value(BattleAmmunitionMode::SemiGuided).unwrap();
        });
        let rules = BattleAimRules {
            woods_damage: false,
            dig_bonus: 2,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: false,
        };
        let unaided = battle_aim_modifiers(&world, shooter, target, index, 4, rules).unwrap();
        assert!(unaided.target_movement > 0);
        let map = world.btech.units()[&shooter].map.unwrap();
        let tagger = world.create(&config, "TAG vehicle".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", &sources[2])
            .unwrap()
            .create(&mut world, tagger)
            .unwrap();
        place_battle_unit(&mut world, tagger, map, 0, 11).unwrap();
        install(&mut world, tagger, false);
        edit(&mut world, tagger, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        release_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(tagger);
        assign_battle_pilot(&mut world, tagger, ObjectId(1)).unwrap();
        refresh_battle_contacts(&mut world, &[tagger]).unwrap();
        select_battle_tag(&mut world, tagger, ObjectId(1), Some(target)).unwrap();
        release_battle_pilot(&mut world, tagger, ObjectId(1)).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
        assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
        let before = world.btech.clone();
        let aided = battle_aim_modifiers(&world, shooter, target, index, 4, rules).unwrap();
        assert_eq!(aided.target_movement, 0);
        assert_eq!(
            unaided.subtotal().unwrap() - aided.subtotal().unwrap(),
            i32::from(unaided.target_movement)
        );
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_aim_modifiers(&restored, shooter, target, index, 4, rules)
                .unwrap()
                .target_movement,
            0
        );
        destroy_battle_vehicle_critical(
            &mut world,
            tagger,
            VehicleCriticalLocation {
                section: BattleVehicleSection::Front,
                slot: 7,
            },
        )
        .unwrap();
        assert_eq!(
            battle_aim_modifiers(&world, shooter, target, index, 4, rules)
                .unwrap()
                .target_movement,
            unaided.target_movement
        );
    }
}
