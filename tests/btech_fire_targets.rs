//! Explicit coordinates share native, Lua and TIC target resolution across supported chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A piloted unit faces an occupied and an empty hex, independently of its saved hex lock.
async fn fixture(
    source: &str,
    mode: HexTargetMode,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Target field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "targets",
        MapAsset::from_cells("3 5\n.0.0.0\n.0.0.0\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let shooter = world.create(&config, "Shooter".into(), Kind::Thing);
    let target = world.create(&config, "Recipient".into(), Kind::Thing);
    for id in [shooter, target] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    }
    UnitTemplate::parse("test", source)
        .unwrap()
        .create(&mut world, shooter)
        .unwrap();
    support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("AS7-D", include_str!("../game/units/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, shooter, map, 1, 4).unwrap();
    place_battle_unit(&mut world, target, map, 1, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, shooter, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let class = if world.btech.vehicles().contains_key(&shooter) {
        "vehicles"
    } else {
        "constructed"
    };
    saved[class][shooter.0.to_string()]["dice"] =
        serde_json::to_value(Dice::seeded([42; 32])).unwrap();
    saved["constructed"][target.0.to_string()]["dice"] =
        serde_json::to_value(Dice::seeded([42; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        HexCoordinate { x: 2, y: 0 },
        mode,
    )
    .unwrap();
    edit_battle_tic(&mut world, shooter, ObjectId(1), 0, TicEdit::Add(vec![0])).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, shooter, target)
}

/// Construct an independent command transaction from identical persisted state.
fn scripts(config: &Config, world: &World) -> Scripts {
    Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap()
}

/// Occupants take unit damage even with a terrain lock; empty hexes keep the saved mode's bonus.
#[tokio::test]
async fn coordinates_share_all_chassis_single_tic_and_restart() {
    let tracked = include_str!("../game/units/Demolisher.toml");
    let wheeled = tracked.replace("movement = \"track\"", "movement = \"wheel\"");
    let hover = tracked.replace("movement = \"track\"", "movement = \"hover\"");
    let stationary = tracked
        .replace("movement = \"track\"", "movement = \"none\"")
        .replace("walk_mp = 5", "walk_mp = 0");
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        tracked,
        wheeled.as_str(),
        hover.as_str(),
        stationary.as_str(),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        for mode in [HexTargetMode::UnitAtHex, HexTargetMode::Hex] {
            let (_dir, config, world, shooter, target) = fixture(source, mode).await;
            persistence::save(&config.database(), &world).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            for y in [0, 1] {
                let lua = scripts(&config, &loaded);
                let call = format!("btech.unit.fire({},1,0,{{x=1,y={y}}})", shooter.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
                let (recipient, x, actual_y, bonus): (Option<i64>, i32, i32, Option<i8>) = lua.eval_callback(&format!("local r={call}; return r.target,r.coordinate.x,r.coordinate.y,r.aim.hex_bonus")).unwrap();
                assert_eq!((x, actual_y), (1, y));
                assert_eq!(recipient, (y == 1).then_some(target.0));
                if y == 0 {
                    assert_eq!(bonus, Some(if mode == HexTargetMode::Hex { -4 } else { 0 }));
                }
                for command in ["fire", "firetic"] {
                    let native = scripts(&config, &world);
                    let text = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("{command} 0 1 {y}"),
                    );
                    assert!(text.contains("You fire"), "{text}");
                    assert_eq!(native.world().btech, lua.world().btech);
                }
                let grouped = scripts(&config, &world);
                let request = FireTarget::Hex {
                    coordinate: HexCoordinate { x: 1, y },
                };
                let reports =
                    fire_battle_tics(&grouped, &config, shooter, ObjectId(1), vec![0], request)
                        .unwrap();
                assert!(reports[0].report.is_some(), "{:?}", reports[0].rejection);
                assert_eq!(grouped.world().btech, lua.world().btech);
                let lua_grouped = scripts(&config, &world);
                let group_call =
                    format!("btech.unit.tic_fire({},1,{{0}},{{x=1,y={y}}})", shooter.0);
                assert!(
                    lua_grouped
                        .eval_callback::<()>(&format!("{group_call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua_grouped.world().btech, world.btech);
                assert!(lua_grouped.drain_outbox().is_empty());
                lua_grouped.eval_callback::<()>(&group_call).unwrap();
                assert_eq!(lua_grouped.world().btech, lua.world().btech);
                let saved = serde_json::to_value(&lua.world().btech).unwrap();
                let before = serde_json::to_value(&world.btech).unwrap();
                let class = if world.btech.vehicles().contains_key(&shooter) {
                    "vehicles"
                } else {
                    "constructed"
                };
                assert_eq!(
                    saved[class][shooter.0.to_string()]["hex_lock"],
                    before[class][shooter.0.to_string()]["hex_lock"]
                );
            }
        }
    }
}

/// Bad coordinates and absent pilots reject without spending a shot or changing the selected lock.
#[tokio::test]
async fn coordinate_rejections_and_cockpit_admission_preserve_state() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
    ] {
        let (_dir, config, world, shooter, _) = fixture(source, HexTargetMode::Hex).await;
        for arguments in ["nope 1", "2147483648 0", "-1 0", "1 999", "1 0 extra"] {
            for command in ["fire", "firetic"] {
                let native = scripts(&config, &world);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command} 0 {arguments}"),
                );
                assert!(!text.contains("You fire"), "{text}");
                assert_eq!(native.world().btech, world.btech);
            }
        }
        let mut unpiloted = world.clone();
        release_battle_pilot(&mut unpiloted, shooter, ObjectId(1)).unwrap();
        for command in ["fire", "firetic"] {
            let native = scripts(&config, &unpiloted);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("{command} bogus bad coordinates"),
            );
            assert!(text.contains("pilot first"), "{text}");
            assert_eq!(native.world().btech, unpiloted.btech);
        }
    }
}

/// Artillery explicit coordinates override saved locks and never select an occupant for immediate damage.
#[tokio::test]
async fn artillery_coordinates_preserve_locks_and_queue_once() {
    for source in [
        include_str!("../game/units/Naga-Prime.toml"),
        include_str!("../game/units/Marksman.toml"),
    ] {
        let (_dir, config, base, shooter, target) = fixture(source, HexTargetMode::Hex).await;
        for selection in 0..3 {
            let mut world = base.clone();
            if selection < 2 {
                select_battle_target(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    (selection == 1).then_some(target),
                )
                .unwrap();
            }
            for y in [0, 1] {
                let lua = scripts(&config, &world);
                let call = format!("btech.unit.fire({},1,0,{{x=1,y={y}}})", shooter.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
                let (recipient, x, actual_y, queued): (Option<i64>, i32, i32, Option<u32>) = lua.eval_callback(&format!("local r={call}; return r.target,r.coordinate.x,r.coordinate.y,r.queued_shot")).unwrap();
                assert_eq!((recipient, x, actual_y), (None, 1, y));
                assert!(queued.is_some());
                assert_eq!(
                    lua.world().btech.constructed_units()[&target],
                    world.btech.constructed_units()[&target]
                );
                for command in ["fire", "firetic"] {
                    let native = scripts(&config, &world);
                    let text = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("{command} 0 1 {y}"),
                    );
                    assert!(text.contains("You fire"), "{text}");
                    assert_eq!(native.world().btech, lua.world().btech);
                }
                let saved = lua.world().clone();
                let before_json = serde_json::to_value(&world.btech).unwrap();
                let after_json = serde_json::to_value(&saved.btech).unwrap();
                let class = if saved.btech.vehicles().contains_key(&shooter) {
                    "vehicles"
                } else {
                    "constructed"
                };
                for field in ["target_lock", "hex_lock"] {
                    assert_eq!(
                        after_json[class][shooter.0.to_string()][field],
                        before_json[class][shooter.0.to_string()][field]
                    );
                }
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
        }
    }
}

/// Change one unit's scenario facts while preserving the same mutation path for both chassis stores.
fn edit_shooter(world: &mut World, id: ObjectId, edit: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, edit).unwrap();
}

/// Native, Lua and Rust TIC attempts reject identically without committing any gameplay effects.
fn assert_mechanical_rejection(config: &Config, world: &World, shooter: ObjectId, reason: &str) {
    world.validate(config).unwrap();
    for command in ["fire", "firetic"] {
        for arguments in ["NOPE", "bad coordinates", "1 0 extra"] {
            let native = scripts(config, world);
            let text = support::run_text(
                &native,
                config,
                ObjectId(1),
                1,
                &format!("{command} 0 {arguments}"),
            );
            assert!(text.contains(reason), "expected {reason}: {text}");
            assert_eq!(native.world().btech, world.btech);
        }
    }
    let lua = scripts(config, world);
    let error = lua
        .eval_callback::<()>(&format!("btech.unit.fire({},1,0,999999)", shooter.0))
        .unwrap_err();
    assert!(error.to_string().contains(reason), "{error}");
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let rejection: String = lua
        .eval_callback(&format!(
            "return btech.unit.tic_fire({},1,{{0}},999999)[1].rejection",
            shooter.0
        ))
        .unwrap();
    assert!(rejection.contains(reason), "{rejection}");
    assert_eq!(lua.world().btech, world.btech);
    let tic = fire_battle_tics(
        &lua,
        config,
        shooter,
        ObjectId(1),
        vec![0],
        Some(ObjectId(999999)),
    )
    .unwrap();
    assert!(tic[0].rejection.as_ref().unwrap().contains(reason));
    assert_eq!(lua.world().btech, world.btech);
}

/// Mechanical admission precedes bad targets for direct, terrain, artillery and TIC firing.
#[tokio::test]
async fn weapon_mechanics_precede_targets_across_chassis_and_restart() {
    let tracked = include_str!("../game/units/Demolisher.toml");
    let wheeled = tracked.replace("movement = \"track\"", "movement = \"wheel\"");
    let hover = tracked.replace("movement = \"track\"", "movement = \"hover\"");
    let stationary = tracked
        .replace("movement = \"track\"", "movement = \"none\"")
        .replace("walk_mp = 5", "walk_mp = 0");
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        tracked,
        wheeled.as_str(),
        hover.as_str(),
        stationary.as_str(),
        include_str!("../game/units/Kestrel.toml"),
        include_str!("../game/units/Naga-Prime.toml"),
        include_str!("../game/units/Marksman.toml"),
    ] {
        let (_dir, config, world, shooter, _) = fixture(source, HexTargetMode::UnitAtHex).await;
        let vehicle = world.btech.vehicles().contains_key(&shooter);
        let readiness = if vehicle {
            world.btech.vehicles()[&shooter]
                .weapon_readiness(0)
                .unwrap()
        } else {
            world.btech.constructed_units()[&shooter]
                .weapon_readiness(0)
                .unwrap()
        };
        let reload = if readiness.weapon.gunnery_skill(true) == "Gunnery-Laser" {
            "still recharging"
        } else {
            "still reloading"
        };
        let mut recycling = world.clone();
        edit_shooter(&mut recycling, shooter, |unit| {
            unit["weapon_recycle"]["0"] = serde_json::json!(5)
        });
        persistence::save(&config.database(), &recycling)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_mechanical_rejection(&config, &restored, shooter, reload);

        let lost = if vehicle {
            serde_json::to_value(
                &world.btech.vehicles()[&shooter].loadout().unwrap().weapons[0].criticals,
            )
            .unwrap()
        } else {
            serde_json::to_value(
                &world.btech.constructed_units()[&shooter]
                    .loadout()
                    .unwrap()
                    .weapons[0]
                    .criticals,
            )
            .unwrap()
        };
        let mut destroyed = world.clone();
        edit_shooter(&mut destroyed, shooter, |unit| {
            unit["lost_criticals"] = lost
        });
        assert_mechanical_rejection(&config, &destroyed, shooter, "weapon has been destroyed");

        let mut stunned = recycling.clone();
        edit_shooter(&mut stunned, shooter, |unit| {
            unit[if vehicle {
                "crew_stun_remaining"
            } else {
                "stun_remaining"
            }] = serde_json::json!(2)
        });
        assert_mechanical_rejection(
            &config,
            &stunned,
            shooter,
            "cannot take actions while stunned",
        );

        let mut spotting = stunned.clone();
        edit_shooter(&mut spotting, shooter, |unit| {
            unit["spotter"] = serde_json::json!(shooter.0)
        });
        assert_mechanical_rejection(&config, &spotting, shooter, "cannot fire while spotting");
        let native = scripts(&config, &spotting);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            "fire 999999 bad coordinates",
        );
        assert!(text.contains("cannot fire while spotting"), "{text}");
        assert_eq!(native.world().btech, spotting.btech);

        if vehicle {
            let mut failed = recycling.clone();
            edit_shooter(&mut failed, shooter, |unit| {
                unit["weapon_failures"]["0"] =
                    serde_json::json!(
                        if readiness.weapon.gunnery_skill(true) == "Gunnery-Ballistic" {
                            "jammed"
                        } else {
                            "shorted"
                        }
                    )
            });
            assert_mechanical_rejection(&config, &failed, shooter, "still unusable");
        }
    }
}

/// Supply rejection remains after native target decoding, even though mechanical checks moved earlier.
#[tokio::test]
async fn empty_ammunition_does_not_hide_native_target_errors() {
    for source in [
        include_str!("../game/units/CPLT-C1.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Naga-Prime.toml"),
        include_str!("../game/units/Marksman.toml"),
    ] {
        let (_dir, config, mut world, shooter, _) = fixture(source, HexTargetMode::UnitAtHex).await;
        edit_shooter(&mut world, shooter, |unit| {
            for rounds in unit["ammunition"].as_array_mut().unwrap() {
                *rounds = serde_json::json!(0);
            }
        });
        world.validate(&config).unwrap();
        let readiness = if let Some(unit) = world.btech.vehicles().get(&shooter) {
            unit.weapon_readiness(0).unwrap()
        } else {
            world.btech.constructed_units()[&shooter]
                .weapon_readiness(0)
                .unwrap()
        };
        assert!(readiness.weapon.profile().ammunition_per_ton > 0);
        assert_eq!(readiness.ammunition, 0);
        assert!(!readiness.ready);
        for command in ["fire", "firetic"] {
            for (args, reason) in [
                ("bad coordinates", "Invalid map coordinates!"),
                ("1 0 extra", "Invalid number of arguments!"),
            ] {
                let native = scripts(&config, &world);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command} 0 {args}"),
                );
                assert!(text.contains(reason), "{text}");
                assert_eq!(native.world().btech, world.btech);
            }
        }
    }
}

/// Anatomy contributes club, limb support and cover restrictions to the same ordered dispatcher.
#[tokio::test]
async fn anatomical_weapon_admission_shares_readiness_and_target_precedence() {
    let (_dir, config, world, shooter, _) = fixture(
        include_str!("../game/units/AS7-D.toml"),
        HexTargetMode::UnitAtHex,
    )
    .await;
    assert_eq!(
        world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons[0]
            .criticals[0]
            .section,
        MechSection::LeftArm
    );
    let mut club = world.clone();
    edit_shooter(&mut club, shooter, |unit| {
        unit["carried_club"] = serde_json::to_value(Arm::Left).unwrap()
    });
    assert!(
        !club.btech.constructed_units()[&shooter]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    assert_mechanical_rejection(&config, &club, shooter, "carrying a club in that arm");

    let mut prone = world.clone();
    edit_shooter(&mut prone, shooter, |unit| {
        unit["posture"] = serde_json::to_value(Posture::Prone).unwrap();
        unit["limb_recycle"]["RightArm"] = serde_json::json!(5);
    });
    assert_mechanical_rejection(&config, &prone, shooter, "Right Arm to prop yourself up");
    edit_shooter(&mut prone, shooter, |unit| {
        unit["limb_recycle"]["LeftArm"] = serde_json::json!(5)
    });
    assert_mechanical_rejection(
        &config,
        &prone,
        shooter,
        "Your Left Arm is still recovering",
    );

    let (_dir, config, mut quad, shooter, _) = fixture(
        include_str!("../game/units/GOL-1H.toml"),
        HexTargetMode::UnitAtHex,
    )
    .await;
    edit_shooter(&mut quad, shooter, |unit| {
        unit["posture"] = serde_json::to_value(Posture::Prone).unwrap();
        unit["flooded_sections"] = serde_json::to_value([
            MechSection::LeftArm,
            MechSection::RightArm,
            MechSection::LeftLeg,
        ])
        .unwrap();
    });
    assert_mechanical_rejection(&config, &quad, shooter, "Quads need at least 3 legs");

    let (_dir, config, mut covered, shooter, _) = fixture(
        include_str!("../game/units/Hunter.toml"),
        HexTargetMode::UnitAtHex,
    )
    .await;
    edit_shooter(&mut covered, shooter, |unit| {
        unit["dig"] = serde_json::to_value(DigState::covered()).unwrap()
    });
    assert_mechanical_rejection(
        &config,
        &covered,
        shooter,
        "Only turret weapons are available while in cover",
    );
}

/// Defensive-only classification precedes target syntax and retains readiness for automatic defense.
#[tokio::test]
async fn defensive_weapon_admission_precedes_targets_without_disabling_ams() {
    for source in [
        include_str!("../game/units/JR7-D.toml").replace("IS.MediumLaser", "IS.LaserAMS"),
        include_str!("../game/units/Demolisher.toml").replace("IS.AC/20", "IS.Anti-MissileSystem"),
    ] {
        let (_dir, config, world, shooter, _) = fixture(&source, HexTargetMode::UnitAtHex).await;
        let readiness = if let Some(unit) = world.btech.vehicles().get(&shooter) {
            unit.weapon_readiness(0).unwrap()
        } else {
            world.btech.constructed_units()[&shooter]
                .weapon_readiness(0)
                .unwrap()
        };
        assert!(readiness.weapon.is_ams());
        assert!(readiness.ready, "{readiness:?}");
        assert_mechanical_rejection(&config, &world, shooter, "That weapon is defensive only!");
    }
}
