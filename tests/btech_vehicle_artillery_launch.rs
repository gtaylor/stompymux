//! Vehicle artillery uses the common host firing transaction and persistent flight queue.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A running Sniper platform faces a nearby empty coordinate.
async fn fixture(flags: &[&str]) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_template(
        flags,
        BattleVehicleTemplate::parse("Marksman", include_str!("../game/mechs/Marksman.toml"))
            .unwrap(),
    )
    .await
}

/// Artillery construction and launch setup is shared by ground and rotorcraft test platforms.
async fn fixture_template(
    flags: &[&str],
    mut template: BattleVehicleTemplate,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Artillery field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let shooter = world.create(&config, "Marksman".into(), Kind::Thing);
    world.objects.get_mut(&shooter).unwrap().home = Some(ObjectId(config.home()));
    for section in template.sections.values_mut() {
        for part in section.criticals.values_mut() {
            if part.equipment.contains("Sniper") {
                part.modes = flags.iter().map(|flag| (*flag).into()).collect();
            }
        }
    }
    create_battle_vehicle(&mut world, shooter, template).unwrap();
    place_battle_unit(&mut world, shooter, map, 1, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, shooter, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 0 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let index = world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.is_artillery())
        .unwrap();
    (dir, config, world, map, shooter, index)
}

#[tokio::test]
async fn vehicle_artillery_native_lua_expenditure_and_queue_replay() {
    let (_dir, config, world, map, shooter, index) = fixture(&[]).await;
    let before = world.btech.vehicles()[&shooter].clone();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report: (bool, u8, u32) = lua.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.launched,r.expenditure.heat,r.queued_shot", shooter.0)).unwrap();
    assert_eq!(report, (true, BattleWeapon::Sniper.profile().heat, 0));
    let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
    assert!(text.contains("You fire Sniper"), "{text}");
    assert_eq!(native.world().btech, lua.world().btech);
    let fired = lua.world().clone();
    let unit = &fired.btech.vehicles()[&shooter];
    assert_eq!(
        before.ammunition().iter().sum::<u16>() - unit.ammunition().iter().sum::<u16>(),
        1
    );
    assert_eq!(
        unit.weapon_heat(),
        f64::from(BattleWeapon::Sniper.profile().heat)
    );
    assert!(unit.weapon_recycle()[&index] > 0);
    assert_eq!(fired.btech.maps()[&map].artillery_shots().len(), 1);
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, fired.btech);
    persistence::save(&config.database(), &fired).await.unwrap();
    let restored = Scripts::new(
        &config,
        Rc::new(RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    for _ in 0..10 {
        assert_eq!(
            advance_artillery_action(&lua, &config, BattleMovementRules::STANDARD.fall).unwrap(),
            advance_artillery_action(&restored, &config, BattleMovementRules::STANDARD.fall)
                .unwrap()
        );
    }
    assert_eq!(lua.world().btech, restored.world().btech);
    assert!(lua.world().btech.maps()[&map].artillery_shots().is_empty());
    lua.world().validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_artillery_rejections_and_failed_callback_are_atomic() {
    let (_dir, config, world, _map, shooter, index) = fixture(&[]).await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    for code in [
        format!("btech.unit.fire({},2,{index})", shooter.0),
        format!("btech.unit.fire({},1,{index},{})", shooter.0, shooter.0),
        format!(
            "btech.unit.fire({},1,{index}); error('rollback launch')",
            shooter.0
        ),
    ] {
        assert!(scripts.eval_callback::<()>(&code).is_err());
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
    }
    let mut rear = world.clone();
    select_battle_hex_target(
        &mut rear,
        shooter,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 2 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let original = rear.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(rear))).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, original);
}

#[tokio::test]
async fn vehicle_artillery_payloads_and_hotload_use_common_launch_rules() {
    for (flag, mode) in [
        ("Cluster", BattleArtilleryMode::Cluster),
        ("Smoke", BattleArtilleryMode::Smoke),
        ("Mine", BattleArtilleryMode::Mine),
    ] {
        let (_dir, config, world, map, shooter, index) = fixture(&[flag]).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
            .unwrap();
        assert_eq!(
            scripts.world().btech.maps()[&map].artillery_shots()[&0]
                .flight
                .mode(),
            mode
        );
    }
    for jam in [false, true] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&[]).await;
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).two_d6() <= 3) == jam)
            .unwrap();
        world
            .btech
            .rewrite_unit_record(shooter, |record| {
                record["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        let ammunition = world.btech.vehicles()[&shooter]
            .ammunition()
            .iter()
            .sum::<u16>();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.unit.hotload({},1,{index})", shooter.0))
            .unwrap();
        let result: (bool, bool, u8) = scripts.eval_callback(&format!("local r=btech.unit.fire({},1,{index}); return r.launched,r.jammed,r.expenditure.heat", shooter.0)).unwrap();
        assert_eq!(
            result,
            (
                !jam,
                jam,
                if jam {
                    0
                } else {
                    BattleWeapon::Sniper.profile().heat
                }
            )
        );
        assert_eq!(
            scripts.world().btech.maps()[&map].artillery_shots().len(),
            usize::from(!jam)
        );
        assert_eq!(
            scripts.world().btech.vehicles()[&shooter]
                .ammunition()
                .iter()
                .sum::<u16>(),
            ammunition - u16::from(!jam)
        );
        let snapshot = scripts.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            snapshot.btech
        );
    }
}

/// Either construction type can supply an automatic visible observation of a vehicle's miss.
#[tokio::test]
async fn vehicle_artillery_correction_uses_mixed_observers_and_replays_aim() {
    for vehicle in [false, true] {
        for (running, friendly) in [(true, true), (false, true), (true, false)] {
            let (_dir, config, mut world, map, shooter, index) = fixture(&["Mine"]).await;
            let observer = world.create(&config, "Observer".into(), Kind::Thing);
            world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
            if vehicle {
                create_battle_vehicle(
                    &mut world,
                    observer,
                    BattleVehicleTemplate::parse(
                        "Demolisher",
                        include_str!("../game/mechs/Demolisher.toml"),
                    )
                    .unwrap(),
                )
                .unwrap();
            } else {
                create_battle_unit(
                    &mut world,
                    observer,
                    BattleTemplate::parse("AS7-D", include_str!("../game/mechs/AS7-D.toml"))
                        .unwrap(),
                )
                .unwrap();
            }
            place_battle_unit(&mut world, observer, map, 2, 1).unwrap();
            if running {
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
                assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
                start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0);
                }
            }
            if !friendly {
                let mut encoded = serde_json::to_value(&world.btech).unwrap();
                encoded[if vehicle { "vehicles" } else { "constructed" }][observer.0.to_string()]
                    ["signature"]["team"] = 99.into();
                world.btech = serde_json::from_value(encoded).unwrap();
            }
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let (hit, aim): (bool, i32) = scripts
                .eval_callback(&format!(
                    "local r=btech.unit.fire({},1,{index}); return r.hit,r.aim.target_number",
                    shooter.0
                ))
                .unwrap();
            assert!(!hit);
            for _ in 0..5 {
                advance_artillery_action(&scripts, &config, BattleMovementRules::STANDARD.fall)
                    .unwrap();
            }
            let midpoint = scripts.world().clone();
            persistence::save(&config.database(), &midpoint)
                .await
                .unwrap();
            let restored = Scripts::new(
                &config,
                Rc::new(RefCell::new(
                    persistence::load(&config.database()).await.unwrap(),
                )),
            )
            .unwrap();
            for _ in 0..5 {
                assert_eq!(
                    advance_artillery_action(&scripts, &config, BattleMovementRules::STANDARD.fall)
                        .unwrap(),
                    advance_artillery_action(
                        &restored,
                        &config,
                        BattleMovementRules::STANDARD.fall
                    )
                    .unwrap()
                );
            }
            assert_eq!(scripts.world().btech, restored.world().btech);
            let correction = u8::from(running && friendly);
            assert_eq!(
                scripts.world().btech.vehicles()[&shooter].artillery_adjustment(),
                correction
            );
            let visible: u8 = scripts
                .eval_callback(&format!(
                    "return btech.unit.state({}).artillery_adjustment",
                    shooter.0
                ))
                .unwrap();
            assert_eq!(visible, correction);
            let mut ready = scripts.world().clone();
            for _ in 0..60 {
                advance_battle_recycle(&mut ready);
            }
            let next = Scripts::new(&config, Rc::new(RefCell::new(ready.clone()))).unwrap();
            let adjusted: i32 = next
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index}).aim.target_number",
                    shooter.0
                ))
                .unwrap();
            assert_eq!(adjusted, aim - i32::from(correction));
            select_battle_hex_target(
                &mut ready,
                shooter,
                ObjectId(1),
                BattleHexCoordinate { x: 2, y: 0 },
                BattleHexTargetMode::Hex,
            )
            .unwrap();
            assert_eq!(ready.btech.vehicles()[&shooter].artillery_adjustment(), 0);
            ready.validate(&config).unwrap();
        }
    }
}

/// Selected observers supply artillery coordinates and invalidate both corrections on retargeting.
#[tokio::test]
async fn vehicle_artillery_explicit_mixed_spotters_share_targets_and_correction_reset() {
    for vehicle in [false, true] {
        let (_dir, config, mut world, map, shooter, index) = fixture(&["Mine"]).await;
        let observer = world.create(&config, "Selected observer".into(), Kind::Thing);
        world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
        if vehicle {
            create_battle_vehicle(
                &mut world,
                observer,
                BattleVehicleTemplate::parse(
                    "Demolisher",
                    include_str!("../game/mechs/Demolisher.toml"),
                )
                .unwrap(),
            )
            .unwrap();
        } else {
            create_battle_unit(
                &mut world,
                observer,
                BattleTemplate::parse("AS7-D", include_str!("../game/mechs/AS7-D.toml")).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, observer, map, 2, 1).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        for _ in 0..10 {
            refresh_battle_contacts(&mut world, &[shooter, observer]).unwrap();
        }
        select_battle_hex_target(
            &mut world,
            observer,
            ObjectId(2),
            BattleHexCoordinate { x: 0, y: 2 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let command = format!(
            "local r=btech.unit.fire({},1,{index}); return r.coordinate.x,r.coordinate.y,r.hit",
            shooter.0
        );
        let result: (i32, i32, bool) = scripts.eval_callback(&command).unwrap();
        assert_eq!((result.0, result.1), (0, 2));
        assert!(!result.2);
        assert_eq!(
            replay.eval_callback::<(i32, i32, bool)>(&command).unwrap(),
            result
        );
        for _ in 0..10 {
            assert_eq!(
                advance_artillery_action(&scripts, &config, BattleMovementRules::STANDARD.fall)
                    .unwrap(),
                advance_artillery_action(&replay, &config, BattleMovementRules::STANDARD.fall)
                    .unwrap()
            );
        }
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(
            scripts.world().btech.vehicles()[&shooter].artillery_adjustment(),
            1
        );
        let mut state = scripts.world().clone();
        select_battle_hex_target(
            &mut state,
            observer,
            ObjectId(2),
            BattleHexCoordinate { x: 1, y: 0 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        assert_eq!(state.btech.vehicles()[&shooter].artillery_adjustment(), 0);
        state.validate(&config).unwrap();
    }
}
