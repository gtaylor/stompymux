//! Operator decoration boundaries, shared native/Lua effects and restart replay.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Both native marker commands use exact diagnostics and ignore arguments after duration.
#[tokio::test]
async fn operator_decoration_reference_argument_replies() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Effects".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "plain",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    for command in ["addfire", "addsmoke"] {
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for args in ["", "0", "0 0"] {
            let reply = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("{command} {args}"),
            );
            assert!(
                reply.contains(&format!(
                    "Error: Invalid number of attributes to {command} command."
                )),
                "{reply}"
            );
        }
        for args in [
            "x 0 1",
            "0 x 1",
            "0 0 x",
            "2147483648 0 1",
            "0 -2147483649 1",
            "0 0 2147483648",
        ] {
            let reply = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("{command} {args}"),
            );
            assert!(
                reply.contains(&format!("Error: Invalid numeric {command} argument.")),
                "{reply}"
            );
        }
        assert_eq!(scripts.world().btech, world.btech);
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("{command} +0 -0 +20 ignored"),
        );
        assert!(reply.contains("at (0,0) with duration of 20s."), "{reply}");
        assert_eq!(
            scripts.world().btech.maps()[&map]
                .decoration(HexCoordinate { x: 0, y: 0 })
                .unwrap()
                .unwrap()
                .remaining,
            20
        );
    }
}

#[tokio::test]
async fn operator_decoration_duration_and_restart() {
    for (command, lua_name, kind) in [
        ("addfire", "add_fire", DecorationKind::Fire),
        ("addsmoke", "add_smoke", DecorationKind::Smoke),
    ] {
        for duration in [i32::MIN, -1, 0, 1, 60, 61, 120, 32767, 65536, i32::MAX] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Effects".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "plain",
                MapAsset::from_cells("1 1\n.0\n").unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
            persistence::save(&config.database(), &world).await.unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let call = format!("btech.map.{lua_name}(1,{},0,0,{duration})", map.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(lua.drain_outbox().is_empty());
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("{command} 0 0 {duration}"),
            );
            assert!(
                output.contains(&format!("with duration of {duration}s.")),
                "{output}"
            );
            assert_eq!(lua.world().btech, native.world().btech);
            let mut saved = native.world().clone();
            let expected = if duration == 0 {
                0
            } else if kind == DecorationKind::Fire {
                i64::from(duration.clamp(-32768, 32767))
            } else {
                i64::from(duration.max(1))
            };
            let coordinate = HexCoordinate { x: 0, y: 0 };
            let effect = saved.btech.maps()[&map]
                .decoration(coordinate)
                .unwrap()
                .unwrap();
            assert_eq!(effect.remaining, expected);
            let object_duration = duration.clamp(-32768, 32767) as i16;
            assert_eq!(effect.object_duration, object_duration);
            assert_eq!(
                effect.next_spread,
                (kind == DecorationKind::Fire && duration != 0).then_some(60)
            );
            persistence::save(&config.database(), &saved).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert_eq!(saved.btech, replay.btech);
            let expiry = if kind == DecorationKind::Fire && expected != 0 {
                60 + i64::from((expected as i16).wrapping_sub(60)).max(1)
            } else {
                expected
            };
            for tick in 1..=if expiry <= 120 { 121 } else { 1 } {
                advance_map_fire(&mut saved).unwrap();
                advance_map_smoke(&mut saved);
                advance_map_fire(&mut replay).unwrap();
                advance_map_smoke(&mut replay);
                let marker = saved.btech.maps()[&map].decoration(coordinate).unwrap();
                if expected == 0 {
                    assert_eq!(marker, Some(effect));
                } else if tick < expiry {
                    assert!(marker.is_some());
                    if kind == DecorationKind::Smoke {
                        assert_eq!(marker.unwrap().remaining, expected - tick);
                        assert_eq!(marker.unwrap().object_duration, object_duration);
                    }
                } else {
                    assert!(
                        marker.is_none(),
                        "{command} duration {duration} tick {tick}"
                    );
                }
                if tick == 1 {
                    persistence::save(&config.database(), &saved).await.unwrap();
                    replay = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(saved.btech, replay.btech);
                }
                if kind == DecorationKind::Fire && tick == 60 && expected != 0 {
                    assert_eq!(
                        marker.unwrap().object_duration,
                        object_duration.wrapping_sub(60)
                    );
                }
            }
            assert_eq!(saved.btech, replay.btech);
            let remaining = saved.btech.maps()[&map].decoration(coordinate).unwrap();
            if duration == 0 {
                assert_eq!(remaining, Some(effect));
            } else if expiry <= 120 {
                assert!(remaining.is_none());
            } else {
                assert_eq!(
                    remaining.unwrap().remaining,
                    if kind == DecorationKind::Smoke {
                        expected - 1
                    } else {
                        expected
                    }
                );
            }
        }
    }
}

#[tokio::test]
async fn operator_decoration_replacement_and_admission() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Effects".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "plain",
        MapAsset::from_cells("1 1\n~2\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    for command in [
        "addfire",
        "addfire 0 0 invalid extra",
        "addsmoke 0 0 2147483648",
        "addfire/no 0 0 1",
        "addfire -1 0 1",
        "addsmoke 32768 0 1",
    ] {
        support::run_text(&scripts, &config, ObjectId(1), 1, command);
        assert_eq!(scripts.world().btech, world.btech);
    }
    assert!(
        add_battle_map_decoration_action(
            &scripts,
            &config,
            ObjectId(2),
            map,
            HexCoordinate { x: 0, y: 0 },
            DecorationKind::Fire,
            1
        )
        .is_err()
    );
    support::run_text(&scripts, &config, ObjectId(1), 1, "addfire 0 0 0");
    support::run_text(&scripts, &config, ObjectId(1), 1, "addsmoke 0 0 -1");
    let mut saved = scripts.world().clone();
    advance_map_smoke(&mut saved);
    assert_eq!(
        saved.btech.maps()[&map].hex(0, 0).unwrap().terrain(),
        Terrain::Water
    );
    assert_eq!(saved.btech.maps()[&map].hex(0, 0).unwrap().water_depth(), 2);
    assert!(!map_fire_pending(&saved));
}

/// Wind changes affect budget spending and future events, never a deadline already scheduled.
#[tokio::test]
async fn changing_wind_preserves_pending_deadlines_and_restart() {
    for (initial_speed, changed_speed, first_tick, budget_after, next_delay) in
        [(0, 40, 60, 100, Some(20)), (40, 0, 20, 60, None)]
    {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Wind".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "plain",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        set_map_wind(&mut world, map, 0, initial_speed).unwrap();
        let coordinate = HexCoordinate { x: 0, y: 0 };
        set_map_decoration(
            &mut world,
            map,
            coordinate,
            Some(BattleDecoration::new(DecorationKind::Fire, 120, None)),
        )
        .unwrap();
        for _ in 0..10 {
            advance_map_fire(&mut world).unwrap();
        }
        set_map_wind(&mut world, map, 0, changed_speed).unwrap();
        let marker = world.btech.maps()[&map]
            .decoration(coordinate)
            .unwrap()
            .unwrap();
        assert_eq!(marker.remaining, 120);
        assert_eq!(marker.next_spread, Some(first_tick - 10));
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        for candidate in [&mut world, &mut replay] {
            for _ in 10..first_tick {
                advance_map_fire(candidate).unwrap();
            }
            let marker = candidate.btech.maps()[&map]
                .decoration(coordinate)
                .unwrap()
                .unwrap();
            assert_eq!(marker.remaining, budget_after);
            assert_eq!(marker.next_spread, next_delay);
            // Further wind changes must not move an admitted burnout deadline either.
            if next_delay.is_none() {
                set_map_wind(candidate, map, 0, 40).unwrap();
            }
            let lifetime = if next_delay.is_some() { 100 } else { 60 };
            for tick in 1..=lifetime {
                advance_map_fire(candidate).unwrap();
                assert_eq!(
                    candidate.btech.maps()[&map]
                        .decoration(coordinate)
                        .unwrap()
                        .is_some(),
                    tick < lifetime
                );
            }
        }
        assert_eq!(world.btech, replay.btech);
    }
}

/// Signed-short subtraction boundaries remain signed until the first spread, including across restart.
#[tokio::test]
async fn extreme_negative_fire_budgets_use_the_current_wind_interval() {
    for (duration, speed, expected) in [
        (-32768, 40, 32748),
        (-32749, 40, 32767),
        (-32748, 40, 1),
        (-32768, 0, 32708),
        (-32709, 0, 32767),
        (-32708, 0, 1),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Signed fire".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "plain",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let coordinate = HexCoordinate { x: 0, y: 0 };
        add_battle_map_decoration_action(
            &scripts,
            &config,
            ObjectId(1),
            map,
            coordinate,
            DecorationKind::Fire,
            duration,
        )
        .unwrap();
        let mut saved = scripts.world().clone();
        for _ in 0..10 {
            advance_map_fire(&mut saved).unwrap();
        }
        set_map_wind(&mut saved, map, 0, speed).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        for candidate in [&mut saved, &mut replay] {
            assert_eq!(
                candidate.btech.maps()[&map]
                    .decoration(coordinate)
                    .unwrap()
                    .unwrap()
                    .remaining,
                i64::from(duration)
            );
            for _ in 10..60 {
                advance_map_fire(candidate).unwrap();
            }
            let marker = candidate.btech.maps()[&map]
                .decoration(coordinate)
                .unwrap()
                .unwrap();
            assert_eq!(marker.remaining, expected);
            assert_eq!(
                marker.next_spread,
                if expected == 1 {
                    None
                } else {
                    Some(candidate.btech.maps()[&map].fire_spread_interval())
                }
            );
            advance_map_fire(candidate).unwrap();
            assert_eq!(
                candidate.btech.maps()[&map]
                    .decoration(coordinate)
                    .unwrap()
                    .is_none(),
                expected == 1
            );
        }
        assert_eq!(saved.btech, replay.btech);
    }
}
