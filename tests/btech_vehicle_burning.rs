//! Vehicle inferno modes, section pulses and crew fire suppression retain saved transaction state.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A placed, shutdown vehicle with an assigned operator and no active scanners.
async fn fixture(stationary: bool) -> (tempfile::TempDir, Config, World, ObjectId) {
    fixture_movement(if stationary {
        VehicleMovement::Stationary
    } else {
        VehicleMovement::Tracked
    })
    .await
}

/// Use the same protection for tracked, wheeled and hover vulnerability checks.
async fn fixture_movement(
    movement: VehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Fire test".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "fire",
        MapAsset::from_cells("1 2\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Burning vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut text = if movement == VehicleMovement::Vtol {
        include_str!("../game/units/Kestrel.toml")
    } else {
        include_str!("../game/units/Demolisher.toml")
    }
    .to_owned();
    if movement == VehicleMovement::Stationary {
        text = text
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0");
    }
    text = text.replace(
        "movement = \"track\"",
        match movement {
            VehicleMovement::Wheeled => "movement = \"wheel\"",
            VehicleMovement::Hover => "movement = \"hover\"",
            _ => "movement = \"track\"",
        },
    );
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", &text).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Alter selected saved fields while retaining domain deserialization validation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// Find an independent stream with a selected leading result.
fn matching_seed(predicate: impl Fn(&mut Dice) -> bool) -> [u8; 32] {
    for number in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&number.to_le_bytes());
        if predicate(&mut Dice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No matching stream");
}

/// Advanced section fires use the ordinary armor damage policy.
fn advanced() -> VehicleImpactRules {
    VehicleImpactRules {
        advanced_fire: true,
        ..VehicleImpactRules::STANDARD
    }
}

#[tokio::test]
async fn advanced_inferno_ignites_once_and_replays_section_pulses_after_shutdown() {
    let (_dir, config, mut world, id) = fixture(false).await;
    let seed = [17; 32];
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
    });
    let original = world.btech.vehicles()[&id].clone();
    let mut dice = Dice::seeded(seed);
    let report = resolve_battle_vehicle_inferno_hit(&mut world, id, 3, advanced()).unwrap();
    assert_eq!(report.damage.len(), 5);
    assert_eq!(report.explosion_roll, None);
    assert_eq!(report.burn_seconds, 0);
    for damage in &report.damage {
        let amount = dice.d6();
        assert_eq!(damage.incoming, u32::from(amount));
        assert_eq!(damage.rolls, [dice.two_d6()]);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&damage.section].armor,
            original.sections()[&damage.section].armor - u16::from(amount)
        );
        assert_eq!(
            world.btech.vehicles()[&id].burning_sections()[&damage.section],
            60
        );
    }
    assert_eq!(world.btech.vehicles()[&id].power(), Power::Off);
    assert_eq!(world.btech.vehicles()[&id].weapon_heat(), 0.0);
    for _ in 0..17 {
        assert!(
            advance_battle_vehicle_fires(&mut world, &config)
                .unwrap()
                .notices
                .is_empty()
        );
    }
    let before = world.btech.clone();
    let repeat = resolve_battle_vehicle_inferno_hit(&mut world, id, 6, advanced()).unwrap();
    assert!(repeat.damage.is_empty());
    assert!(
        repeat
            .notices
            .iter()
            .any(|notice| notice.text.contains("More burning jelly"))
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for remaining in (1..=43).rev() {
        let notices = advance_battle_vehicle_fires(&mut world, &config).unwrap();
        assert_eq!(
            notices,
            advance_battle_vehicle_fires(&mut restored, &config).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        if remaining > 1 {
            assert!(notices.notices.is_empty());
        } else {
            assert!(
                notices
                    .notices
                    .iter()
                    .any(|notice| notice.text.contains("takes damage from the fire"))
            );
        }
    }
    for section in original.sections().keys() {
        let amount = dice.d6();
        dice.two_d6();
        assert_eq!(
            world.btech.vehicles()[&id].burning_sections().get(section),
            if amount == 1 { None } else { Some(&60) }
        );
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn standard_explosions_and_stationary_jelly_preserve_distinct_rules() {
    let (_dir, config, base, id) = fixture(false).await;
    for roll in [8, 9] {
        let mut world = base.clone();
        let seed = matching_seed(|dice| dice.two_d6() == roll);
        edit(&mut world, id, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
        });
        let before = world.btech.vehicles()[&id].clone();
        let mut dice = Dice::seeded(seed);
        dice.two_d6();
        let report =
            resolve_battle_vehicle_inferno_hit(&mut world, id, 1, VehicleImpactRules::STANDARD)
                .unwrap();
        assert_eq!(report.explosion_roll, Some(roll));
        assert_eq!(report.explosion.is_some(), roll == 9);
        assert!(world.btech.vehicles()[&id].burning_sections().is_empty());
        assert_eq!(world.btech.vehicles()[&id].is_destroyed(), roll == 9);
        if roll == 8 {
            assert_eq!(world.btech.vehicles()[&id].sections(), before.sections());
        } else {
            assert!(
                world.btech.vehicles()[&id]
                    .sections()
                    .values()
                    .all(|section| section.internal == 0)
            );
        }
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        world.validate(&config).unwrap();
    }
    let (_dir, config, base, id) = fixture(true).await;
    for rules in [advanced(), VehicleImpactRules::STANDARD] {
        let mut world = base.clone();
        let report = resolve_battle_vehicle_inferno_hit(&mut world, id, 3, rules).unwrap();
        assert_eq!(report.burn_seconds, 360);
        assert!(
            report.damage.is_empty()
                && report.explosion.is_none()
                && report.explosion_roll.is_none()
        );
        let _ = resolve_battle_vehicle_inferno_hit(&mut world, id, 1, rules).unwrap();
        assert_eq!(world.btech.vehicles()[&id].inferno_remaining(), 540);
        let map = world.btech.vehicles()[&id].position().unwrap().map;
        assert!(battle_hex_illuminated(&world, map, HexCoordinate { x: 0, y: 1 }).unwrap());
        assert!(begin_battle_vehicle_extinguishing(&mut world, id, ObjectId(1)).is_err());
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 0..539 {
            assert!(
                advance_battle_vehicle_fires(&mut restored, &config)
                    .unwrap()
                    .notices
                    .is_empty()
            );
        }
        let notices = advance_battle_vehicle_fires(&mut restored, &config).unwrap();
        assert!(
            notices
                .notices
                .iter()
                .any(|notice| notice.text.contains("fires finally die"))
        );
        assert_eq!(restored.btech, base.btech);
        assert!(!battle_hex_illuminated(&restored, map, HexCoordinate { x: 0, y: 1 }).unwrap());
    }
}

#[tokio::test]
async fn fire_pulses_extinguish_on_one_and_discard_destroyed_sections_without_damage() {
    let (_dir, config, mut base, id) = fixture(false).await;
    let seed = matching_seed(|dice| dice.d6() == 1);
    edit(&mut base, id, |unit| {
        unit["burning_sections"] = serde_json::json!({"turret":1});
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap();
    });
    let mut world = base.clone();
    let armor = world.btech.vehicles()[&id].sections()[&VehicleSection::Turret].armor;
    let notices = advance_battle_vehicle_fires(&mut world, &config).unwrap();
    assert!(world.btech.vehicles()[&id].burning_sections().is_empty());
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&VehicleSection::Turret].armor,
        armor - 1
    );
    assert!(
        notices
            .notices
            .iter()
            .any(|notice| notice.text.contains("finally goes out"))
    );
    let mut world = base.clone();
    let internal = world.btech.vehicles()[&id].sections()[&VehicleSection::Turret].internal;
    let _ = damage_battle_vehicle_phase(
        &mut world,
        id,
        VehicleSection::Turret,
        internal,
        DamagePhase::Internal,
    )
    .unwrap();
    assert!(
        advance_battle_vehicle_fires(&mut world, &config)
            .unwrap()
            .notices
            .is_empty()
    );
    let mut dice = Dice::seeded(seed);
    dice.d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    assert!(world.btech.vehicles()[&id].burning_sections().is_empty());
    let mut world = base;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let before = world.btech.clone();
    assert!(
        advance_battle_vehicle_fires(&mut world, &config)
            .unwrap()
            .notices
            .is_empty()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn extinguishing_shares_native_lua_countdown_and_callback_rollback() {
    let (_dir, config, mut base, id) = fixture(false).await;
    edit(&mut base, id, |unit| {
        unit["burning_sections"] = serde_json::json!({"front":60})
    });
    let lua = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let call = format!("btech.unit.extinguish({},1)", id.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<()>(&call).unwrap();
    let output = support::run_text(&native, &config, ObjectId(1), 1, "extinguish");
    assert!(output.contains("begin to extinguish"), "{output}");
    assert_eq!(lua.world().btech, native.world().btech);
    let mut world = lua.world().clone();
    assert_eq!(world.btech.vehicles()[&id].extinguishing(), Some(120));
    let before = world.btech.clone();
    assert!(begin_battle_vehicle_extinguishing(&mut world, id, ObjectId(1)).is_err());
    assert_eq!(world.btech, before);
    // Keep the one section burning through both scheduled pulses.
    let seed = matching_seed(|dice| {
        let a = dice.d6();
        dice.two_d6();
        let b = dice.d6();
        a > 1 && b > 1
    });
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
    });
    for _ in 0..119 {
        let _ = advance_battle_vehicle_fires(&mut world, &config).unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let notices = advance_battle_vehicle_fires(&mut world, &config).unwrap();
    assert_eq!(
        notices,
        advance_battle_vehicle_fires(&mut restored, &config).unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    assert!(
        notices
            .notices
            .iter()
            .any(|notice| notice.text.contains("dowse the fire"))
    );
    assert!(world.btech.vehicles()[&id].burning_sections().is_empty());
    assert_eq!(world.btech.vehicles()[&id].extinguishing(), None);
    assert!(begin_battle_vehicle_extinguishing(&mut world, id, ObjectId(1)).is_err());
    let mut running = base.clone();
    edit(&mut running, id, |unit| {
        unit["power"] = serde_json::to_value(Power::Running).unwrap()
    });
    assert!(begin_battle_vehicle_extinguishing(&mut running, id, ObjectId(1)).is_err());
    assert!(begin_battle_vehicle_extinguishing(&mut base, id, ObjectId(2)).is_err());
}

#[tokio::test]
async fn invalid_burn_snapshots_are_rejected_and_hull_fire_preserves_occupants() {
    let (_dir, config, base, id) = fixture(false).await;
    for (field, value) in [
        ("burning_sections", serde_json::json!({"front":0})),
        ("burning_sections", serde_json::json!({"front":61})),
        ("extinguishing", serde_json::json!(0)),
        ("extinguishing", serde_json::json!(121)),
        ("inferno_remaining", serde_json::json!(2147483648u32)),
    ] {
        let mut saved = serde_json::to_value(&base.btech).unwrap();
        saved["vehicles"][id.0.to_string()][field] = value;
        assert!(serde_json::from_value::<BtechState>(saved).is_err());
    }
    let seed = matching_seed(|dice| {
        if dice.d6() != 1 {
            return false;
        }
        dice.two_d6();
        dice.two_d6() < 8
    });
    let mut world = base;
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap();
        unit["burning_sections"] = serde_json::json!({"front":1});
        unit["sections"]["front"]["armor"] = 0.into();
        unit["sections"]["front"]["internal"] = 1.into();
    });
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let report = advance_battle_vehicle_fires(&mut world, &config).unwrap();
    assert!(report.character_injuries.is_empty());
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
}

/// A stopped vehicle keeps the heartbeat active and retries the exact pulse after a failed save.
#[tokio::test]
async fn shutdown_vehicle_fire_retries_failed_server_commit() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, id) = fixture(false).await;
        let seed = matching_seed(|dice| dice.d6() == 1);
        edit(&mut world, id, |unit| {
            unit["burning_sections"] = serde_json::json!({"front":1});
            unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap();
        });
        assert!(battle_contact_observers(&world).is_empty());
        let mut expected = world.clone();
        let _ = advance_battle_vehicle_fires(&mut expected, &config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_fire BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'vehicle fire failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua, mut heartbeats) = support::start(&config, Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let failed = persistence::load(&config.database()).await.unwrap();
        assert_eq!(failed.btech.vehicles()[&id], world.btech.vehicles()[&id]);
        sqlx::raw_sql("DROP TRIGGER deny_fire;").execute(&mut sql).await.unwrap();
        let committed = heartbeats.until_saved(&config, 5, |committed| committed.btech.vehicles()[&id].burning_sections().is_empty()).await;
        assert_eq!(committed.btech.vehicles()[&id], expected.btech.vehicles()[&id]);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}

/// Hull destruction and virtual crew loss cancel every thermal event without another pulse.
#[tokio::test]
async fn vehicle_destruction_cancels_fire_jelly_and_extinguishing() {
    let (_dir, config, base, id) = fixture(false).await;
    for crew in [false, true] {
        let mut world = base.clone();
        edit(&mut world, id, |unit| {
            unit["burning_sections"] = serde_json::json!({"front":1,"turret":1});
            unit["inferno_remaining"] = 180.into();
            unit["extinguishing"] = 1.into();
        });
        if crew {
            let _ = injure_battle_tactical_pilot(&mut world, id, 6, false).unwrap();
        } else {
            let _ = damage_battle_vehicle_phase(
                &mut world,
                id,
                VehicleSection::Front,
                100,
                DamagePhase::Internal,
            )
            .unwrap();
        }
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.is_destroyed());
        assert!(unit.burning_sections().is_empty());
        assert_eq!(unit.extinguishing(), None);
        assert_eq!(unit.inferno_remaining(), 0);
        let before = world.btech.clone();
        assert!(
            advance_battle_vehicle_fires(&mut world, &config)
                .unwrap()
                .notices
                .is_empty()
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}

/// Every environmental threshold uses the same motive, armor and ignition stages as combat.
#[tokio::test]
async fn terrain_fire_checks_share_motive_damage_ignition_and_exact_dice() {
    for (movement, modifier) in [
        (VehicleMovement::Tracked, 0),
        (VehicleMovement::Wheeled, 2),
        (VehicleMovement::Hover, 4),
    ] {
        let (_dir, config, base, id) = fixture_movement(movement).await;
        for roll in 2..=12 {
            let mut world = base.clone();
            let seed = matching_seed(|dice| dice.two_d6() == roll);
            edit(&mut world, id, |unit| {
                unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
            });
            let mut dice = Dice::seeded(seed);
            dice.two_d6();
            let report =
                resolve_battle_vehicle_fire_exposure(&mut world, id, advanced().criticals).unwrap();
            assert_eq!(report.roll, roll);
            assert_eq!(report.adjusted, roll + modifier);
            match roll + modifier {
                0..=7 => {
                    assert!(report.effects.notices.is_empty());
                }
                8 | 9 => {
                    let motive_roll = dice.two_d6();
                    assert_eq!(report.motive_roll, Some(motive_roll));
                    let (penalty, loss, immobilized) = match motive_roll + modifier {
                        8 | 9 => (1, 0.0, false),
                        10 | 11 => (2, 10.75, false),
                        12.. => (0, 0.0, true),
                        _ => (0, 0.0, false),
                    };
                    let unit = &world.btech.vehicles()[&id];
                    assert_eq!(unit.piloting_damage(), penalty);
                    assert_eq!(unit.motive_speed_loss(), loss);
                    assert_eq!(unit.immobilized(), immobilized);
                }
                adjusted => {
                    assert_eq!(report.effects.damage.len(), 5);
                    for damage in &report.effects.damage {
                        assert_eq!(damage.incoming, u32::from(dice.d6()));
                        assert_eq!(damage.rolls, [dice.two_d6()]);
                    }
                    if adjusted < 12 {
                        for _ in 0..3 {
                            dice.d6();
                        }
                    }
                    assert_eq!(
                        world.btech.vehicles()[&id].burning_sections().len(),
                        if adjusted >= 12 { 5 } else { 0 }
                    );
                }
            }
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            world.validate(&config).unwrap();
        }
    }
}

/// A single admitted fire attack completes every section after its first fatal hull hit.
#[tokio::test]
async fn fatal_fire_continues_section_damage_and_replays_wreck_pulses() {
    let (_dir, config, base, id) = fixture(false).await;
    for ignition in [false, true] {
        let mut world = base.clone();
        let seed = matching_seed(|dice| {
            if !ignition && dice.two_d6() != 10 {
                return false;
            }
            dice.d6();
            dice.two_d6();
            dice.two_d6() <= 7
        });
        edit(&mut world, id, |unit| {
            unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap();
            unit["sections"]["left"]["armor"] = 0.into();
            unit["sections"]["left"]["internal"] = 1.into();
        });
        let damage = if ignition {
            resolve_battle_vehicle_inferno_hit(&mut world, id, 1, advanced())
                .unwrap()
                .damage
        } else {
            resolve_battle_vehicle_fire_exposure(&mut world, id, advanced().criticals)
                .unwrap()
                .effects
                .damage
        };
        assert_eq!(damage.len(), 5);
        assert!(damage[0].unit_destroyed);
        for hit in &damage[1..] {
            assert!(hit.unit_destroyed);
            assert!(hit.absorbed > 0);
        }
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.is_destroyed());
        assert_eq!(unit.burning_sections().len(), if ignition { 5 } else { 0 });
        world.validate(&config).unwrap();
        if !ignition {
            continue;
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let before = world.btech.vehicles()[&id].clone();
        for _ in 0..60 {
            assert_eq!(
                advance_battle_vehicle_fires(&mut world, &config).unwrap(),
                advance_battle_vehicle_fires(&mut restored, &config).unwrap()
            );
        }
        assert_eq!(world.btech, restored.btech);
        assert!(
            !world.btech.vehicles()[&id]
                .burning_sections()
                .contains_key(&VehicleSection::Left)
        );
        for section in [
            VehicleSection::Right,
            VehicleSection::Front,
            VehicleSection::Rear,
            VehicleSection::Turret,
        ] {
            assert!(
                world.btech.vehicles()[&id].sections()[&section].armor
                    < before.sections()[&section].armor
            );
        }
        world.validate(&config).unwrap();
    }
}

/// Mobile blast checks apply at zero heat and can ignite or explode surviving wreck material.
#[tokio::test]
async fn blast_heat_checks_zero_heat_and_existing_wrecks() {
    for stationary in [false, true] {
        let (_dir, config, base, id) = fixture(stationary).await;
        for wreck in [false, true] {
            for heat in [0, 2] {
                for advanced_fire in [false, true] {
                    for roll in [8, 9, 12] {
                        let mut world = base.clone();
                        if wreck {
                            let _ = damage_battle_vehicle_phase(
                                &mut world,
                                id,
                                VehicleSection::Front,
                                100,
                                DamagePhase::Internal,
                            )
                            .unwrap();
                        }
                        let seed = matching_seed(|dice| dice.two_d6() == roll);
                        edit(&mut world, id, |unit| {
                            unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
                        });
                        let before = world.clone();
                        let rules = VehicleImpactRules {
                            advanced_fire,
                            ..VehicleImpactRules::STANDARD
                        };
                        let result =
                            resolve_battle_vehicle_heat_exposure(&mut world, id, heat, rules)
                                .unwrap();
                        if stationary {
                            assert_eq!(result.burn_seconds, i64::from(heat) * 6);
                            assert!(result.fire.is_none() && result.explosion_roll.is_none());
                            assert_eq!(
                                roll_unit_dice(&mut world, id, 1).unwrap(),
                                [Dice::seeded(seed).d6()]
                            );
                        } else if advanced_fire {
                            assert_eq!(result.fire.as_ref().unwrap().roll, roll);
                            if roll == 12 {
                                assert_eq!(
                                    world.btech.vehicles()[&id].burning_sections().len(),
                                    if wreck { 4 } else { 5 }
                                );
                            }
                        } else {
                            assert_eq!(result.explosion_roll, Some(roll));
                            assert_eq!(result.explosion.is_some(), roll > 8);
                            if roll > 8 {
                                assert!(
                                    world.btech.vehicles()[&id]
                                        .sections()
                                        .values()
                                        .all(|section| section.internal == 0)
                                );
                            }
                        }
                        let mut replay = before;
                        assert_eq!(
                            resolve_battle_vehicle_heat_exposure(&mut replay, id, heat, rules)
                                .unwrap(),
                            result
                        );
                        if stationary {
                            roll_unit_dice(&mut replay, id, 1).unwrap();
                        }
                        assert_eq!(world.btech, replay.btech);
                        world.validate(&config).unwrap();
                    }
                }
            }
        }
    }
}

/// Heat, terrain exposure and inferno share private emergency feedback from burning sections.
async fn aircraft_fire_feedback_matrix(source: usize) {
    let (_dir, config, mut base, id) = fixture_movement(VehicleMovement::Vtol).await;
    base.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let passenger = base.create(&config, "Passenger".into(), Kind::Player);
    base.objects.get_mut(&passenger).unwrap().location = Some(id);
    base.objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    edit(&mut base, id, |state| {
        state["power"] = serde_json::to_value(Power::Running).unwrap();
        state["vtol_flight"] = serde_json::to_value(VtolFlight {
            fall: None,
            phase: VtolFlightPhase::Airborne,
            altitude: 2.0,
            vertical_speed: 0.0,
        })
        .unwrap();
        for section in state["sections"].as_object_mut().unwrap().values_mut() {
            section["armor"] = 0.into();
        }
    });
    let mut policy = advanced();
    policy.criticals.table = VehicleCriticalTable::Standard;
    policy.criticals.vtol_table = None;
    policy.criticals.enabled = true;
    // One VM pair per shard; each seed candidate installs its world, per the
    // sandbox-reuse convention, instead of booting fresh VMs.
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let replay = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let resolve = |scripts: &Scripts| -> Vec<PilotNotice> {
        match source {
            0 => {
                resolve_battle_vehicle_heat_exposure_action(scripts, &config, id, 5, policy)
                    .unwrap()
                    .pilot_notices
            }
            1 => {
                resolve_battle_vehicle_fire_exposure_action(scripts, &config, id, policy.criticals)
                    .unwrap()
                    .effects
                    .pilot_notices
            }
            2 => {
                resolve_battle_vehicle_inferno_hit_action(scripts, &config, id, 2, policy)
                    .unwrap()
                    .pilot_notices
            }
            _ => {
                advance_battle_vehicle_fires_action(scripts, &config)
                    .unwrap()
                    .pilot_notices
            }
        }
    };
    let mut covered = false;
    for seed in 0..=255 {
        let mut world = base.clone();
        edit(&mut world, id, |state| {
            state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
        });
        if source == 3 {
            edit(&mut world, id, |state| {
                state["burning_sections"] = serde_json::json!({"left":1})
            });
        }
        support::install(&scripts, world.clone());
        let expected = resolve(&scripts);
        if expected.is_empty() {
            continue;
        }
        let output = scripts.drain_outbox();
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
            expected
                .iter()
                .map(|notice| (notice.pilot, notice.text.clone()))
                .collect::<Vec<_>>()
        );
        assert!(actual.iter().all(|(who, _)| *who == ObjectId(1)));
        assert!(output.iter().any(|(who, _)| *who == passenger));
        support::install(&replay, world);
        assert_eq!(resolve(&replay), expected);
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(output, replay.drain_outbox());
        covered = true;
        break;
    }
    assert!(
        covered,
        "fire source {source} must exercise an emergency roll"
    );
}

#[tokio::test]
async fn aircraft_fire_feedback_reaches_only_the_pilot_heat() {
    aircraft_fire_feedback_matrix(0).await;
}

#[tokio::test]
async fn aircraft_fire_feedback_reaches_only_the_pilot_fire() {
    aircraft_fire_feedback_matrix(1).await;
}

#[tokio::test]
async fn aircraft_fire_feedback_reaches_only_the_pilot_inferno() {
    aircraft_fire_feedback_matrix(2).await;
}

#[tokio::test]
async fn aircraft_fire_feedback_reaches_only_the_pilot_advance() {
    aircraft_fire_feedback_matrix(3).await;
}
