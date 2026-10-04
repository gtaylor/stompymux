//! Ground motion cadence, continuous positions, transactional controls and explicit transition
//! stops, plus perception, target locks, aiming, shot resolution, falls and standing, balance,
//! stagger, water, glancing fire, ammunition hazards, overheat, Lua firing, and direct-fire
//! weapon parity through heavy gauss recoil.

use crate::btech_motion_common::{
    RULES, balance_hit, balance_skill, computer_skill, configured_shot_rules,
    expected_grass_miss_rolls, fall_rules, fixture, fixture_assets, fixture_source, lock_fixture,
    optical_aim_rules, overheat_due, overheat_rules, shot_fixture, shot_rules, shot_seed,
    shot_skill, single_critical_seed, stagger_fixture, stagger_hit, stagger_rules, stand_fixture,
    water_fall_seed, water_fixture,
};
use crate::support;
use crate::support::{install, restore_database, snapshot_database};
use stompymux_rs::{
    BattleMovementRules, BattlePower, BattleTemplate, Kind, MapAsset, ObjectId, Scripts,
    advance_battle_motion, advance_battle_units, assign_battle_pilot, create_battle_map,
    create_battle_unit, persistence, place_battle_unit, set_battle_heading, set_battle_speed,
    start_battle_unit, stop_battle_unit,
};

#[tokio::test]
async fn acceleration_crosses_hexes_and_restart_preserves_fractional_motion() {
    let (_dir, config, mut world, id) = fixture('.').await;
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for step in 1..=20 {
        assert!(advance_battle_motion(&mut world, RULES).unwrap().is_empty());
        if step == 10 {
            persistence::save(&config.database(), &world).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, world.btech);
            world = loaded;
        }
    }
    let motion = world.btech.constructed_units()[&id].motion().unwrap();
    assert!((motion.speed - 118.25).abs() < 1e-10);
    assert!((motion.point.y - 3.075).abs() < 1e-10);
    assert_eq!(
        world.btech.constructed_units()[&id].position().unwrap().y,
        3
    );
    let mut fasa = world.clone();
    set_battle_heading(&mut fasa, id, ObjectId(1), 90.0).unwrap();
    advance_battle_motion(
        &mut fasa,
        BattleMovementRules {
            fasa_turning: true,
            slowdown: 2,
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(
        fasa.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .heading,
        0.0
    );
    let mut turning = world.clone();
    set_battle_heading(&mut turning, id, ObjectId(1), 90.0).unwrap();
    advance_battle_motion(&mut turning, RULES).unwrap();
    assert!(
        (turning.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .heading
            - 5.5)
            .abs()
            < 1e-10
    );
    let mut moving_shutdown = world.clone();
    stop_battle_unit(&mut moving_shutdown, id, ObjectId(1), RULES.fall).unwrap();
    assert_eq!(
        moving_shutdown.btech.constructed_units()[&id].posture(),
        stompymux_rs::BattlePosture::Prone
    );
    set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
    for _ in 0..21 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    persistence::save(&config.database(), &world).await.unwrap();
}

#[tokio::test]
async fn turning_terrain_limits_and_invalid_controls_do_not_teleport_units() {
    let (_dir, _config, mut world, id) = fixture('%').await;
    let before = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(1), f64::NAN).is_err());
    assert!(set_battle_speed(&mut world, id, ObjectId(1), -118.25).is_err());
    assert!(set_battle_heading(&mut world, id, ObjectId(2), 90.0).is_err());
    assert_eq!(world.btech, before);
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..6 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert_eq!(
        world.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .heading,
        90.0
    );
    set_battle_heading(&mut world, id, ObjectId(1), 0.1).unwrap();
    for _ in 0..6 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert_eq!(
        world.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .heading,
        0.1
    );
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert!((world.btech.constructed_units()[&id].motion().unwrap().speed - 59.125).abs() < 1e-10);
    let (_dir, _config, mut wall, id) = fixture('=').await;
    let point = wall.btech.constructed_units()[&id].motion().unwrap().point;
    set_battle_speed(&mut wall, id, ObjectId(1), 20.0).unwrap();
    assert!(advance_battle_motion(&mut wall, RULES).unwrap().is_empty());
    assert_ne!(
        wall.btech.constructed_units()[&id].motion().unwrap().point,
        point
    );
    assert!(wall.btech.constructed_units()[&id].motion().unwrap().speed > 0.0);
}

#[tokio::test]
async fn controls_rollback_and_map_edges_stop_without_losing_position() {
    let (_dir, config, world, id) = fixture('.').await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.speed({},1,100); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "speed run");
    assert!(text.contains("Desired speed changed to 118 KPH."), "{text}");
    let mut stopped = false;
    for _ in 0..100 {
        let notices = advance_battle_motion(&mut scripts.world_mut(), RULES).unwrap();
        if !notices.is_empty() {
            assert!(notices[0].text.contains("Map edge"));
            stopped = true;
            break;
        }
    }
    assert!(stopped);
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .desired_speed,
        0.0
    );
    let world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Exercise the saved map percentage through normal load/save boundaries.
async fn movement_rate(config: &stompymux_rs::Config, world: &mut stompymux_rs::World, rate: i64) {
    use sqlx::Connection;
    persistence::save(&config.database(), world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_maps SET move_mod=?")
        .bind(rate)
        .execute(&mut sql)
        .await
        .unwrap();
    *world = persistence::load(&config.database()).await.unwrap();
}

#[tokio::test]
async fn saved_map_rates_scale_displacement_and_crossed_obstacles_cannot_be_skipped() {
    for (rate, scale) in [(50, 0.5), (200, 2.0), (0, 1.0), (-100, 1.0)] {
        let (_dir, config, mut world, id) = fixture('.').await;
        movement_rate(&config, &mut world, rate).await;
        let before = world.btech.constructed_units()[&id].motion().unwrap().point;
        set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
        assert!(advance_battle_motion(&mut world, RULES).unwrap().is_empty());
        let after = world.btech.constructed_units()[&id].motion().unwrap().point;
        assert!((before.range(after).unwrap() - 118.25 / 20.0 / 645.0 * scale).abs() < 1e-12);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
    for hazard in [false, true] {
        let mut source = String::from("12 12\n");
        for y in 0..12 {
            source.push_str(&if hazard && y == 3 { "=9" } else { ".0" }.repeat(12));
            source.push('\n');
        }
        let (_dir, config, mut world, id) = fixture_source(&source).await;
        movement_rate(&config, &mut world, 50000).await;
        let before = world.btech.constructed_units()[&id].motion().unwrap().point;
        set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
        let notices = advance_battle_motion(&mut world, RULES).unwrap();
        if hazard {
            assert!(notices[0].text.contains("too steep"));
            assert_eq!(
                world.btech.constructed_units()[&id].position().unwrap().y,
                4
            );
            assert!(
                world.btech.constructed_units()[&id]
                    .motion()
                    .unwrap()
                    .point
                    .y
                    > before.y - 2.0
            );
        } else {
            assert!(notices.is_empty());
            assert_eq!(
                world.btech.constructed_units()[&id].position().unwrap().y,
                0
            );
            assert!(
                advance_battle_motion(&mut world, RULES).unwrap()[0]
                    .text
                    .contains("Map edge")
            );
        }
    }
}

#[tokio::test]
async fn damage_clamps_live_controls_and_native_speed_uses_remaining_mobility() {
    use stompymux_rs::{BattleSection, CriticalLocation, destroy_battle_critical};
    let (_dir, config, mut world, id) = fixture('.').await;
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::LeftLeg,
            slot: 0,
        },
    )
    .unwrap();
    let motion = world.btech.constructed_units()[&id].motion().unwrap();
    assert_eq!(motion.speed, 59.125);
    assert_eq!(motion.desired_speed, 59.125);
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 118.25).is_err());
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "speed run");
    assert!(text.contains("Desired speed changed to 59 KPH."), "{text}");
    assert_eq!(
        scripts
            .eval_callback::<f64>(&format!(
                "return btech.unit.state({}).mobility.maximum_speed",
                id.0
            ))
            .unwrap(),
        59.125
    );
    destroy_battle_critical(
        &mut scripts.world_mut(),
        id,
        CriticalLocation {
            section: BattleSection::RightLeg,
            slot: 0,
        },
    )
    .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .speed,
        0.0
    );
    assert!(set_battle_heading(&mut scripts.world_mut(), id, ObjectId(1), 90.0).is_err());
    let world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn weapon_expenditure_is_atomic_and_recycle_pauses_through_shutdown_and_restart() {
    use stompymux_rs::{BattleWeapon, advance_battle_recycle, spend_battle_weapon};
    let (_dir, config, mut world, id) = fixture('.').await;
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    let missile = loadout
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Srm4)
        .unwrap();
    let used = spend_battle_weapon(&mut world, id, ObjectId(1), missile).unwrap();
    assert_eq!(
        used.ammunition,
        vec![stompymux_rs::BattleAmmunitionDraw {
            bin_index: 0,
            rounds: 1
        }]
    );
    assert_eq!(used.heat, 3);
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[24]);
    let before = world.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), missile).is_err());
    assert!(spend_battle_weapon(&mut world, id, ObjectId(2), 0).is_err());
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), 999).is_err());
    assert_eq!(world.btech, before.btech);
    let laser = spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
    assert!(laser.ammunition.is_empty());
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..20 {
        assert!(advance_battle_recycle(&mut world).is_empty());
    }
    assert_eq!(
        world.btech.constructed_units()[&id].weapon_recycle()[&missile],
        15
    );
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    for _ in 0..14 {
        assert!(advance_battle_recycle(&mut world).is_empty());
    }
    let notices = advance_battle_recycle(&mut world);
    assert_eq!(notices.len(), 1);
    assert!(notices[0].text.contains("SRM-4"));
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(missile)
            .unwrap()
            .ready
    );
    assert_eq!(world.btech.constructed_units()[&id].weapon_recycle()[&0], 5);
    for _ in 0..24 {
        spend_battle_weapon(&mut world, id, ObjectId(1), missile).unwrap();
        for _ in 0..15 {
            advance_battle_recycle(&mut world);
        }
    }
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[0]);
    let before = world.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), missile).is_err());
    assert_eq!(world.btech, before.btech);
}

#[tokio::test(flavor = "current_thread")]
async fn stationary_weapon_recycle_uses_server_commit_and_retries_failed_ticks() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture('.').await;
        stompymux_rs::spend_battle_weapon(&mut world,id,ObjectId(1),0).unwrap();
        for _ in 0..18 {stompymux_rs::advance_battle_recycle(&mut world);}
        persistence::save(&config.database(),&world).await.unwrap();
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_recycle BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'recycle failure'); END;").execute(&mut sql).await.unwrap();
        let (_address,shutdown,task,_lua,mut heartbeats)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].weapon_recycle()[&0],2);
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].heat().stored,3.0);
        sqlx::query("DROP TRIGGER deny_recycle").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].weapon_recycle().is_empty()).await;
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn weapon_heat_survives_restart_and_cools_after_shutdown() {
    use stompymux_rs::{advance_battle_heat, spend_battle_weapon};
    let (_dir, config, mut world, id) = fixture('.').await;
    spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
    assert_eq!(world.btech.constructed_units()[&id].heat().stored, 3.0);
    let before = world.btech.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), 0).is_err());
    assert_eq!(world.btech, before);
    advance_battle_heat(&mut world);
    let heat = world.btech.constructed_units()[&id].heat();
    assert_eq!(heat.excess, 0.0);
    assert_eq!(heat.stored, 3.0 - 10.0 / 30.0);
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.constructed_units()[&id].heat(), heat);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..10 {
        advance_battle_heat(&mut world);
    }
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.heat(), stompymux_rs::BattleHeat::default());
    assert!(!unit.heat_active(&world));
    assert_eq!(unit.weapon_recycle()[&0], 20);
}

#[tokio::test]
async fn damaged_engine_movement_and_lost_sinks_change_heat_rates() {
    use stompymux_rs::{BattleSystem, destroy_battle_critical};
    let (_dir, _config, mut world, id) = fixture('.').await;
    let systems = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems;
    let engine = systems
        .iter()
        .find(|critical| critical.system == BattleSystem::Engine)
        .unwrap();
    destroy_battle_critical(&mut world, id, engine.location).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .production,
        5.0
    );
    let sink = systems
        .iter()
        .find(|critical| critical.system == BattleSystem::HeatSink)
        .unwrap();
    destroy_battle_critical(&mut world, id, sink.location).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        9.0
    );
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .production,
        7.0
    );
    set_battle_speed(&mut world, id, ObjectId(1), -10.0).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .production,
        6.0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_unit_receives_server_cooling_without_active_recycle() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id) = fixture('.').await;
            stompymux_rs::spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
            stop_battle_unit(
                &mut world,
                id,
                ObjectId(1),
                stompymux_rs::BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _lua, mut heartbeats) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            heartbeats
                .until_saved(&config, 5, |saved| {
                    let unit = &saved.btech.constructed_units()[&id];
                    assert_eq!(unit.power(), BattlePower::Off);
                    assert_eq!(unit.weapon_recycle()[&0], 20);
                    unit.heat().stored < 3.0
                })
                .await;
            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

#[tokio::test]
async fn heat_slows_motion_gradually_preserves_throttle_and_recovers_after_cooling() {
    let (_dir, config, mut world, id) = fixture('.').await;
    // Compose weapon expenditure without cooling to prepare a deliberately hot unit.
    for _ in 0..12 {
        stompymux_rs::spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
        for _ in 0..20 {
            stompymux_rs::advance_battle_recycle(&mut world);
        }
    }
    stompymux_rs::advance_battle_heat(&mut world);
    assert_eq!(world.btech.constructed_units()[&id].heat().excess, 26.0);
    let mut reversing = world.clone();
    set_battle_speed(&mut reversing, id, ObjectId(1), -59.125).unwrap();
    for _ in 0..5 {
        advance_battle_motion(&mut reversing, RULES).unwrap();
    }
    let reverse_motion = reversing.btech.constructed_units()[&id].motion().unwrap();
    assert!((reverse_motion.speed + 16.125).abs() < 1e-10);
    assert_eq!(reverse_motion.desired_speed, -59.125);

    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        118.25 / 20.0
    );
    for _ in 0..10 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    let motion = world.btech.constructed_units()[&id].motion().unwrap();
    assert!((motion.speed - 32.25).abs() < 1e-10);
    assert_eq!(motion.desired_speed, 118.25);
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap(),
        motion
    );
    for _ in 0..150 {
        stompymux_rs::advance_battle_heat(&mut world);
    }
    advance_battle_motion(&mut world, RULES).unwrap();
    let recovered = world.btech.constructed_units()[&id].motion().unwrap();
    assert!(recovered.speed > motion.speed);
    assert_eq!(recovered.desired_speed, motion.desired_speed);
}

#[tokio::test]
async fn occupied_environment_changes_heat_and_survives_restart() {
    for (tile, flags, temperature, production, dissipation) in [
        ("&0", 0, 20, 5.0, 10.0),
        ("~1", 0, 20, 0.0, 10.0),
        ("~2", 0, 20, 0.0, 16.0),
        (".0", 0, 100, 0.0, 10.0),
        (".0", 2, 50, 0.0, 10.0),
        (".0", 2, 51, 0.0, 9.0),
        (".0", 2, 61, 0.0, 8.0),
        (".0", 2, -30, 0.0, 10.0),
        (".0", 2, -31, 0.0, 11.0),
        ("~2", 2, -41, 0.0, 18.0),
    ] {
        let source = format!(
            "12 12\n{}{flags}: 100 {temperature}\n",
            format!("{}\n", tile.repeat(12)).repeat(12)
        );
        let (_dir, config, mut world, id) = fixture_source(&source).await;
        let rates = world.btech.constructed_units()[&id].heat_rates(&world);
        assert_eq!(rates.production, production);
        assert_eq!(rates.dissipation, dissipation);
        stompymux_rs::spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
        stompymux_rs::advance_battle_heat(&mut world);
        let heat = world.btech.constructed_units()[&id].heat();
        assert!((heat.stored - (3.0 - (dissipation - production) / 30.0)).abs() < 1e-10);
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].heat_rates(&world),
            rates
        );
        assert_eq!(world.btech.constructed_units()[&id].heat(), heat);
    }
}

#[tokio::test]
async fn shallow_water_counts_only_surviving_leg_sinks() {
    use stompymux_rs::{BattleSection, CriticalLocation, destroy_battle_critical};
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let sink = template.sections[&BattleSection::Head].criticals[&3].clone();
    for section in [BattleSection::LeftLeg, BattleSection::RightLeg] {
        for slot in [4, 5] {
            template
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(slot, sink.clone());
        }
    }
    let source = format!("12 12\n{}", format!("{}\n", "~1".repeat(12)).repeat(12));
    let (_dir, _config, mut world, id) = fixture_assets(&source, template).await;
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        14.0
    );
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::LeftLeg,
            slot: 4,
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        12.0
    );
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::Head,
            slot: 3,
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        11.0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn environmental_heat_wakes_stationary_server_simulation() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let source = format!(
                "12 12\n{}2: 100 127\n",
                format!("{}\n", ".0".repeat(12)).repeat(12)
            );
            let (_dir, config, mut world, id) = fixture_source(&source).await;
            let engine = world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .systems
                .iter()
                .find(|part| part.system == stompymux_rs::BattleSystem::Engine)
                .unwrap()
                .location;
            stompymux_rs::destroy_battle_critical(&mut world, id, engine).unwrap();
            assert_eq!(world.btech.constructed_units()[&id].heat().stored, 0.0);
            let mut off = world.clone();
            stop_battle_unit(
                &mut off,
                id,
                ObjectId(1),
                stompymux_rs::BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
            assert!(!off.btech.constructed_units()[&id].heat_active(&off));
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _lua, mut heartbeats) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            heartbeats
                .until_saved(&config, 5, |saved| {
                    saved.btech.constructed_units()[&id].heat().stored > 0.0
                })
                .await;
            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

#[tokio::test]
async fn tactical_injuries_recover_without_profiles_and_sixth_hit_ends_the_unit() {
    use stompymux_rs::{BattleDice, Flag, injure_battle_tactical_pilot};
    let (_dir, config, mut world, id) = fixture('.').await;
    let seed = (0..=255)
        .map(|byte| [byte; 32])
        .find(|seed| BattleDice::seeded(*seed).two_d6() == 2)
        .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["recoveries"]["1"] = serde_json::json!({
        "mode":{"kind":"tactical","injuries":0}, "remaining":0,
        "pain_resistance":false,"toughness":false,"dice":BattleDice::seeded(seed)
    });
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(world.btech.characters().is_empty());
    let before = world.btech.clone();
    assert_eq!(
        injure_battle_tactical_pilot(&mut world, id, 0, false)
            .unwrap()
            .injuries,
        0
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    assert!(injure_battle_tactical_pilot(&mut world, id, 1, false).is_err());
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    stompymux_rs::spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
    let report = injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap();
    assert_eq!(report.injuries, 1);
    let check = report.consciousness.unwrap();
    assert_eq!((check.target, check.roll, check.conscious), (3, 2, false));
    assert!(world.btech.unconscious(ObjectId(1)));
    assert!(report.notice(id).is_some());
    let report = injure_battle_tactical_pilot(&mut world, id, 2, false).unwrap();
    assert_eq!(report.injuries, 3);
    assert!(report.consciousness.is_none());
    assert_eq!(world.btech.recoveries()[&ObjectId(1)].remaining, 30);
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.btech.clone();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, before);
    let armor = world.btech.constructed_units()[&id].sections().clone();
    let report = injure_battle_tactical_pilot(&mut world, id, 255, false).unwrap();
    assert!(report.killed);
    assert_eq!(report.injuries, 127);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.is_destroyed());
    assert_eq!(unit.sections(), &armor);
    assert_eq!(unit.power(), BattlePower::Off);
    assert_eq!(unit.pilot(), None);
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert!(unit.weapon_recycle().is_empty());
    assert!(!world.btech.unconscious(ObjectId(1)));
    assert!(world.btech.characters().is_empty());
    // The wreck's cockpit cannot be claimed again, so it cannot be restarted.
    let error = assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap_err();
    assert_eq!(error.to_string(), "Unit is destroyed");
    assert!(start_battle_unit(&mut world, id, ObjectId(1), true).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn tactical_head_impact_applies_armor_and_pilot_injury_together() {
    let (_dir, _config, mut world, id) = fixture('.').await;
    let hit = stompymux_rs::BattleHit {
        section: stompymux_rs::BattleSection::Head,
        rear_armor: false,
        through_armor_critical: false,
        crew_stun: true,
    };
    let before = world.btech.clone();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::InCharacter);
    assert!(
        stompymux_rs::resolve_battle_tactical_impact(&mut world, id, hit, 1, fall_rules()).is_err()
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(stompymux_rs::Flag::InCharacter);
    let report =
        stompymux_rs::resolve_battle_tactical_impact(&mut world, id, hit, 1, fall_rules()).unwrap();
    assert_eq!(report.pilot_injuries.len(), 1);
    assert_eq!(report.pilot_injuries[0].injuries, 1);
    assert_eq!(
        world.btech.constructed_units()[&id].sections()[&stompymux_rs::BattleSection::Head].armor,
        6
    );
    assert_eq!(world.btech.constructed_units()[&id].pilot_injuries(), 1);
    assert!(report.impact.pending_effects.is_empty());
    assert_eq!(world.btech.constructed_units()[&id].stun_remaining(), 10);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text.contains("momentarily stunned"))
    );
}

#[tokio::test]
async fn stun_refreshes_expires_after_restart_and_blocks_expenditure_without_stopping_motion() {
    use stompymux_rs::{advance_battle_stun, spend_battle_weapon, stun_battle_unit};
    let (_dir, config, mut world, id) = fixture('.').await;
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    stun_battle_unit(&mut world, id).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.stun_remaining(), 10);
    assert_eq!(unit.motion().unwrap().speed, 118.25);
    assert_eq!(unit.motion().unwrap().desired_speed, 118.25 * 2.0 / 3.0);
    assert!(!unit.weapon_readiness(0).unwrap().ready);
    let before = world.btech.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), 0).is_err());
    assert_eq!(world.btech, before);
    advance_battle_motion(&mut world, RULES).unwrap();
    assert!(world.btech.constructed_units()[&id].motion().unwrap().speed > 118.25 * 2.0 / 3.0);
    for _ in 0..4 {
        assert!(advance_battle_stun(&mut world).is_empty());
    }
    stun_battle_unit(&mut world, id).unwrap();
    assert_eq!(world.btech.constructed_units()[&id].stun_remaining(), 10);
    for _ in 0..9 {
        assert!(advance_battle_stun(&mut world).is_empty());
    }
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.constructed_units()[&id].stun_remaining(), 1);
    assert_eq!(
        advance_battle_stun(&mut world)[0].text,
        "You recover from your stunning experience!"
    );
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
}

#[tokio::test]
async fn stun_preserves_reverse_throttle_and_expires_silently_after_shutdown() {
    let (_dir, _config, mut world, id) = fixture('.').await;
    set_battle_speed(&mut world, id, ObjectId(1), -70.0).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    stompymux_rs::stun_battle_unit(&mut world, id).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .desired_speed,
        -70.0
    );
    let (_dir, _config, mut world, id) = fixture('.').await;
    stompymux_rs::stun_battle_unit(&mut world, id).unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..10 {
        assert!(stompymux_rs::advance_battle_stun(&mut world).is_empty());
    }
    assert_eq!(world.btech.constructed_units()[&id].stun_remaining(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn stationary_stun_recovery_uses_server_commit_and_retries_failed_saves() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture('.').await;
        stompymux_rs::stun_battle_unit(&mut world, id).unwrap();
        for _ in 0..8 { stompymux_rs::advance_battle_stun(&mut world); }
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_stun BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'stun failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_stun").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].stun_remaining() == 0).await;
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn stun_throttle_change_precedes_a_simultaneous_hip_critical() {
    let (_dir, _config, mut world, id) = fixture('.').await;
    balance_skill(&mut world);
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    for seed in 0..=255 {
        let mut trial = world.clone();
        trial
            .btech
            .set_unit_dice(id, stompymux_rs::BattleDice::seeded([seed; 32]))
            .unwrap();
        let hit = stompymux_rs::BattleHit {
            section: stompymux_rs::BattleSection::LeftLeg,
            rear_armor: false,
            through_armor_critical: true,
            crew_stun: true,
        };
        let report =
            stompymux_rs::resolve_battle_tactical_impact(&mut trial, id, hit, 1, fall_rules())
                .unwrap();
        if report.impact.criticals.len() != 1
            || !report.impact.criticals.iter().any(|(location, _)| {
                location.section == stompymux_rs::BattleSection::LeftLeg && location.slot == 0
            })
        {
            continue;
        }
        let unit = &trial.btech.constructed_units()[&id];
        assert_eq!(unit.mobility().maximum_speed, 59.125);
        assert_eq!(unit.motion().unwrap().desired_speed, 59.125);
        assert_eq!(unit.stun_remaining(), 10);
        return;
    }
    panic!("No deterministic seed exercised the hip critical");
}

#[tokio::test]
async fn aim_breakdown_tracks_turning_equipment_and_heat_without_mutation() {
    use stompymux_rs::{
        BattleAimRules, BattleSection, CriticalLocation, battle_aim_modifiers,
        destroy_battle_critical,
    };
    let (_dir, config, mut world, shooter) = fixture('.').await;
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    let target = world.create(&config, "Aim target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 7).unwrap();
    let rules = BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: true,
        extended_movement: true,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    };
    stompymux_rs::refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let base = battle_aim_modifiers(&world, shooter, target, 0, 4, rules).unwrap();
    assert_eq!(base.distance, 2.0);
    assert_eq!(base.target_movement, -4);
    assert_eq!(base.target_lock, 2);
    assert_eq!(base.subtotal(), Some(2));
    set_battle_heading(&mut world, shooter, ObjectId(1), 90.0).unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .attacker_movement,
        1
    );
    set_battle_speed(&mut world, shooter, ObjectId(1), 10.0).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .attacker_movement,
        2
    );
    destroy_battle_critical(
        &mut world,
        shooter,
        CriticalLocation {
            section: BattleSection::LeftArm,
            slot: 1,
        },
    )
    .unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .mounting_section,
        1
    );
    destroy_battle_critical(
        &mut world,
        shooter,
        CriticalLocation {
            section: BattleSection::LeftArm,
            slot: 0,
        },
    )
    .unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .mounting_section,
        4
    );
    destroy_battle_critical(
        &mut world,
        shooter,
        CriticalLocation {
            section: BattleSection::Head,
            slot: 1,
        },
    )
    .unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .sensors,
        2
    );
    destroy_battle_critical(
        &mut world,
        shooter,
        CriticalLocation {
            section: BattleSection::Head,
            slot: 4,
        },
    )
    .unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .sensors,
        75
    );
    for _ in 0..10 {
        stompymux_rs::spend_battle_weapon(&mut world, shooter, ObjectId(1), 0).unwrap();
        for _ in 0..20 {
            stompymux_rs::advance_battle_recycle(&mut world);
        }
    }
    stompymux_rs::advance_battle_heat(&mut world);
    let before = world.btech.clone();
    assert_eq!(
        battle_aim_modifiers(&world, shooter, target, 0, 4, rules)
            .unwrap()
            .heat,
        3
    );
    assert!(battle_aim_modifiers(&world, shooter, target, 999, 4, rules).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn torso_and_arm_facing_are_guarded_and_persistent() {
    use stompymux_rs::{BattleTorso, flip_battle_arms, rotate_battle_torso};
    let (_dir, config, mut world, id) = fixture('.').await;
    rotate_battle_torso(&mut world, id, ObjectId(1), BattleTorso::Left).unwrap();
    let before = world.btech.clone();
    assert!(rotate_battle_torso(&mut world, id, ObjectId(1), BattleTorso::Left).is_err());
    assert!(flip_battle_arms(&mut world, id, ObjectId(2)).is_err());
    assert_eq!(world.btech, before);
    rotate_battle_torso(&mut world, id, ObjectId(1), BattleTorso::Right).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].facing().torso,
        BattleTorso::Center
    );
    rotate_battle_torso(&mut world, id, ObjectId(1), BattleTorso::Right).unwrap();
    flip_battle_arms(&mut world, id, ObjectId(1)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let facing = world.btech.constructed_units()[&id].facing();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.constructed_units()[&id].facing(), facing);
    assert!(facing.arms_flipped);
    assert_eq!(facing.torso, BattleTorso::Right);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let before = world.btech.clone();
    assert!(flip_battle_arms(&mut world, id, ObjectId(1)).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn facing_commands_validate_arguments_and_preserve_pose_on_failure() {
    use stompymux_rs::BattleTorso;
    let (_dir, config, world, id) = fixture('.').await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let command = |text| support::run_text(&scripts, &config, ObjectId(1), 1, text);
    assert!(command("rottorso L").contains("rotate your torso left"));
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .facing()
            .torso,
        BattleTorso::Left
    );
    let before = scripts.world().btech.clone();
    assert!(command("rottorso left").contains("beyond 60"));
    assert!(command("rottorso sideways").contains("Usage:"));
    assert!(command("rottorso").contains("Usage:"));
    assert!(command("fliparms extra").contains("Usage:"));
    assert!(command("fliparms/quiet").contains("no switches"));
    assert!(command("rottorso/quiet r").contains("no switches"));
    assert_eq!(scripts.world().btech, before);
    assert!(command("rottorso r").contains("rotate your torso right"));
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .facing()
            .torso,
        BattleTorso::Center
    );
    assert!(command("fliparms").contains("BACKWARD"));
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
    assert!(command("fliparms").contains("FORWARD"));
    command("shutdown");
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    let stopped = scripts.world().btech.clone();
    command("rottorso right");
    command("fliparms");
    assert_eq!(scripts.world().btech, stopped);
}

#[tokio::test]
async fn terrain_los_queries_follow_placement_and_leave_world_unchanged() {
    let (_dir, config, mut world, id) = fixture('.').await;
    let target = world.create(&config, "LOS target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    assert!(stompymux_rs::battle_unit_terrain_los(&world, id, target).is_err());
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    place_battle_unit(&mut world, target, map, 5, 9).unwrap();
    let before = world.btech.clone();
    let report = stompymux_rs::battle_unit_terrain_los(&world, id, target).unwrap();
    assert!(!report.blocked);
    assert_eq!(report.woods, 0);
    assert_eq!(world.btech, before);
    let other = world.create(&config, "Other LOS map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, other, 0, 0).unwrap();
    assert!(stompymux_rs::battle_unit_terrain_los(&world, id, target).is_err());
}

/// Perception traces live terrain and spatial range, and a query never acquires contacts.
#[tokio::test]
async fn perception_query_composes_live_terrain_and_spatial_range_without_acquiring_contacts() {
    use stompymux_rs::{
        BattleDetectionChannel, BattleLight, battle_perceive, configure_battle_perception,
        set_battle_map_visibility,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let target = world.create(&config, "Perceived target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    place_battle_unit(&mut world, target, map, 5, 9).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 3).unwrap();
    let before = world.btech.clone();
    // Four hexes away is inside the sensor band, where darkness does not matter.
    let perceived = battle_perceive(&world, id, target).unwrap().unwrap();
    assert_eq!(perceived.channel, BattleDetectionChannel::Sensors);
    assert!(perceived.identified);
    assert!(!perceived.probed);
    assert_eq!(perceived.aim_modifier, 0);
    assert_eq!(
        perceived.range,
        stompymux_rs::battle_unit_range(&world, id, target).unwrap()
    );
    // Without the band, an unlit target beyond night visibility is not seen at all.
    let mut sight = world.clone();
    configure_battle_perception(&mut sight, 0);
    assert_eq!(battle_perceive(&sight, id, target).unwrap(), None);
    // Within night visibility sight reaches it, at +1 for darkness.
    set_battle_map_visibility(&mut sight, map, BattleLight::Night, 4).unwrap();
    let seen = battle_perceive(&sight, id, target).unwrap().unwrap();
    assert_eq!(seen.channel, BattleDetectionChannel::Sight);
    assert_eq!(seen.aim_modifier, 1);
    assert!(sight.btech.constructed_units()[&id].contacts().is_empty());
    assert_eq!(world.btech, before);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for unit in [id, target] {
        stompymux_rs::remove_battle_unit(&mut world, unit, ObjectId(config.start())).unwrap();
    }
    stompymux_rs::reload_battle_map(
        &mut world,
        map,
        "forest.map",
        MapAsset::from_cells(&format!(
            "12 12\n{}",
            format!("{}\n", "`0".repeat(12)).repeat(12)
        ))
        .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 5, 5).unwrap();
    place_battle_unit(&mut world, target, map, 5, 9).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 60).unwrap();
    // Three intervening light-forest hexes break the clear line for both sensors and sight.
    assert!(
        stompymux_rs::battle_unit_terrain_los(&world, id, target)
            .unwrap()
            .woods
            >= 3
    );
    assert_eq!(battle_perceive(&world, id, target).unwrap(), None);
}

/// Saved light and visibility change an occupied battlefield's perception without touching terrain.
#[tokio::test]
async fn saved_map_visibility_changes_occupied_battlefields_and_perception_queries() {
    use sqlx::{Connection, SqliteConnection};
    use stompymux_rs::{
        BattleDetectionChannel, BattleLight, battle_perceive, configure_battle_perception,
        set_battle_map_visibility,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Weather target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, map, 5, 9).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER prevent_terrain_change BEFORE DELETE ON btech_map_hexes BEGIN SELECT RAISE(ABORT,'terrain must not change'); END").execute(&mut sql).await.unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 3).unwrap();
    assert_eq!(
        battle_perceive(&world, id, target)
            .unwrap()
            .unwrap()
            .channel,
        BattleDetectionChannel::Sensors
    );
    // Sight alone cannot reach an unlit target four hexes away in three hexes of night visibility.
    let mut sight = world.clone();
    configure_battle_perception(&mut sight, 0);
    assert_eq!(battle_perceive(&sight, id, target).unwrap(), None);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(loaded.btech.maps()[&map].maximum_visibility, 24);
    let before = world.btech.clone();
    assert!(set_battle_map_visibility(&mut world, map, BattleLight::Day, 61).is_err());
    assert_eq!(world.btech, before);
    // A stored ceiling can differ from the setter's computed value and must be honored.
    sqlx::query("UPDATE btech_maps SET max_visibility=2 WHERE dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    // Even the sensor band needs a clear line inside the battlefield ceiling.
    assert_eq!(battle_perceive(&loaded, id, target).unwrap(), None);
    sqlx::query("UPDATE btech_maps SET light=3 WHERE dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

/// Hidden hostile searches roll the observer's saved dice and resume them after a restart;
/// missing targets are rejected, and ordinary, distant or close targets consume no dice.
#[tokio::test]
async fn hidden_contact_search_resumes_observer_dice_after_restart_and_rejects_missing_targets() {
    use stompymux_rs::{
        BattleContactRules, BattleContactTransition as Transition, BattleDetection, BattleDice,
        battle_perception_factor, update_battle_contact,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let target = world.create(&config, "Detection target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let dice = |world: &stompymux_rs::World| {
        serde_json::to_value(&world.btech).unwrap()["constructed"][id.0.to_string()]["dice"].clone()
    };
    let rules = BattleContactRules {
        hostile: true,
        hidden: true,
        perception: 7,
        acquire: true,
    };
    let before = world.btech.clone();
    assert!(update_battle_contact(&mut world, id, ObjectId(i64::MAX), rules).is_err());
    assert_eq!(world.btech, before);
    // Beyond five hexes a hidden hostile unit cannot be found without a probe.
    place_battle_unit(&mut world, target, map, 5, 11).unwrap();
    let before = world.btech.clone();
    let distant = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(distant.transition, Transition::Unseen);
    assert_eq!(
        distant.detection,
        Some(BattleDetection {
            detected: false,
            threshold: 0,
            roll: None,
        })
    );
    assert_eq!(world.btech, before);
    // Ordinary targets are acquired at once without a search.
    place_battle_unit(&mut world, target, map, 5, 9).unwrap();
    let mut ordinary = world.clone();
    let update = update_battle_contact(
        &mut ordinary,
        id,
        target,
        BattleContactRules {
            hidden: false,
            ..rules
        },
    )
    .unwrap();
    assert_eq!(update.transition, Transition::Acquired);
    assert_eq!(
        update.detection,
        Some(BattleDetection {
            detected: true,
            threshold: 0,
            roll: None,
        })
    );
    assert_eq!(dice(&ordinary), dice(&world));
    // Four hexes behind the observer: rear weight 40, quartered perception factor, 98% proximity.
    let threshold = 40 * battle_perception_factor(7) / 100 / 4 * 98;
    assert_eq!(threshold, 686);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).die(10_000).unwrap() >= threshold)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let mut stream = BattleDice::seeded([seed; 32]);
    let before = world.btech.clone();
    let first = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(first.transition, Transition::Unseen);
    assert_eq!(
        first.detection,
        Some(BattleDetection {
            detected: false,
            threshold,
            roll: Some(stream.die(10_000).unwrap()),
        })
    );
    assert_eq!(dice(&world), serde_json::to_value(&stream).unwrap());
    assert_eq!(
        world.btech.constructed_units()[&target],
        before.constructed_units()[&target]
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let expected = update_battle_contact(&mut world, id, target, rules).unwrap();
    let resumed = update_battle_contact(&mut loaded, id, target, rules).unwrap();
    assert_eq!(resumed, expected);
    let roll = stream.die(10_000).unwrap();
    assert_eq!(
        expected.detection,
        Some(BattleDetection {
            detected: roll < threshold,
            threshold,
            roll: Some(roll),
        })
    );
    assert_eq!(loaded.btech, world.btech);
    // Inside three hexes the search succeeds automatically without consuming dice.
    place_battle_unit(&mut world, target, map, 5, 7).unwrap();
    let before = dice(&world);
    let close = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(close.transition, Transition::Acquired);
    assert_eq!(
        close.detection,
        Some(BattleDetection {
            detected: true,
            threshold: 7 * 99,
            roll: None,
        })
    );
    assert_eq!(dice(&world), before);
}

/// The sensor command and Lua share one read-only perception summary and refuse arguments.
#[tokio::test]
async fn sensor_command_and_lua_report_read_only_perception() {
    use stompymux_rs::{BattleLight, set_battle_map_visibility};
    let (_dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 10).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let run = |player, line| {
        support::run_text(
            &scripts,
            &config,
            ObjectId(player),
            u64::try_from(player).unwrap(),
            line,
        )
    };
    let expected = [
        "Sensors: 15 hexes in any light or weather",
        "Sight:   10 hexes at night, +1 to hit unless the target is lit; lit targets to 30",
        "Probe:   none",
        "Radar:   none",
    ]
    .join("\r\n");
    let before = scripts.world().btech.clone();
    // Any occupant may read the summary.
    for player in [2, 1] {
        let text = run(player, "sensor");
        assert!(text.contains(&expected), "{text}");
    }
    for invalid in ["sensor V L", "sensor v", "sensor verbose"] {
        assert!(
            run(1, invalid)
                .contains("Sensors are automatic; the sensor command takes no arguments."),
            "{invalid}"
        );
    }
    assert_eq!(scripts.world().btech, before);
    let (text, range, sensors, running): (String, u16, String, bool) = scripts
        .eval_callback(&format!(
            "local report = btech.unit.perception({}); return report.text, report.sensor_range, report.sensors, report.running",
            id.0
        ))
        .unwrap();
    assert_eq!(text, expected);
    assert_eq!((range, sensors.as_str(), running), (15, "ready", true));
    assert_eq!(scripts.world().btech, before);
    run(1, "shutdown");
    let text = run(1, "sensor");
    assert!(
        text.contains("Your unit is shut down; nothing is perceived until it starts."),
        "{text}"
    );
}

/// Contacts are acquired, retained while any channel reaches, lost when none does, and cleared
/// by administrative placement or removal.
#[tokio::test]
async fn contacts_acquire_retain_lose_and_clear_on_administrative_placement() {
    use stompymux_rs::{
        BattleContact, BattleContactRules, BattleContactTransition as Transition, BattleDetection,
        BattleLight, BattleMapPerceptionFlag, set_battle_map_perception, set_battle_map_visibility,
        update_battle_contact,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Contact target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 7).unwrap();
    let rules = BattleContactRules {
        hostile: true,
        hidden: false,
        perception: 7,
        acquire: true,
    };
    let before = world.btech.clone();
    let update = update_battle_contact(
        &mut world,
        id,
        target,
        BattleContactRules {
            acquire: false,
            ..rules
        },
    )
    .unwrap();
    assert_eq!(update.transition, Transition::Unseen);
    assert_eq!(update.detection, None);
    assert_eq!(world.btech, before);
    assert!(update_battle_contact(&mut world, id, id, rules).is_err());
    assert_eq!(world.btech, before);
    let update = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(update.transition, Transition::Acquired);
    assert_eq!(
        update.detection,
        Some(BattleDetection {
            detected: true,
            threshold: 0,
            roll: None,
        })
    );
    assert_eq!(
        world.btech.constructed_units()[&id].contacts()[&target],
        BattleContact { identified: true }
    );
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let before = world.btech.clone();
    let update = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(update.transition, Transition::Retained);
    assert!(update.detection.is_none());
    assert_eq!(world.btech, before);
    // References are checked before malformed contact state can be persisted.
    let mut corrupt = world.clone();
    corrupt
        .btech
        .rewrite_unit_record(id, |record| {
            record["contacts"]["999999"] = serde_json::json!({"identified":true});
        })
        .unwrap();
    assert!(corrupt.validate(&config).is_err());
    // Weather alone cannot hide a target inside the sensor band.
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    let update = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(update.transition, Transition::Retained);
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    let update = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(update.transition, Transition::Lost);
    assert!(update.detection.is_none());
    assert!(world.btech.constructed_units()[&id].contacts().is_empty());
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    assert_eq!(
        update_battle_contact(&mut world, id, target, rules)
            .unwrap()
            .transition,
        Transition::Acquired
    );
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    assert!(world.btech.constructed_units()[&id].contacts().is_empty());
    update_battle_contact(&mut world, id, target, rules).unwrap();
    stompymux_rs::remove_battle_unit(&mut world, target, ObjectId(config.start())).unwrap();
    assert!(world.btech.constructed_units()[&id].contacts().is_empty());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Saved perception switches gate contacts and leave unrelated sensor flag bits untouched.
#[tokio::test]
async fn saved_perception_disable_flags_gate_contacts_and_preserve_other_sensor_bits() {
    use sqlx::Connection;
    use stompymux_rs::{
        BattleContactRules, BattleContactTransition as Transition, BattleLight,
        BattleMapPerceptionFlag as Flag, set_battle_map_perception, set_battle_map_visibility,
        update_battle_contact,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Disabled scanner target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, map, 5, 7).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_maps SET sensor_flags=128 WHERE dbref=?")
        .bind(map.0)
        .execute(&mut sql)
        .await
        .unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    let rules = BattleContactRules {
        hostile: true,
        hidden: false,
        perception: 7,
        acquire: true,
    };
    assert_eq!(
        update_battle_contact(&mut world, id, target, rules)
            .unwrap()
            .transition,
        Transition::Acquired
    );
    // With no visibility, only the sensor band reaches the target.
    set_battle_map_perception(&mut world, map, Flag::Sensors, false).unwrap();
    assert_eq!(world.btech.maps()[&map].sensor_flags, 129);
    assert!(world.btech.maps()[&map].perception_disabled(Flag::Sensors));
    let lost = update_battle_contact(&mut world, id, target, rules).unwrap();
    assert_eq!(lost.transition, Transition::Lost);
    assert!(lost.detection.is_none());
    let before = world.btech.clone();
    assert_eq!(
        update_battle_contact(&mut world, id, target, rules)
            .unwrap()
            .transition,
        Transition::Unseen
    );
    assert_eq!(world.btech, before);
    set_battle_map_perception(&mut world, map, Flag::Radar, false).unwrap();
    assert_eq!(world.btech.maps()[&map].sensor_flags, 161);
    set_battle_map_perception(&mut world, map, Flag::Probes, false).unwrap();
    assert_eq!(world.btech.maps()[&map].sensor_flags, 225);
    set_battle_map_perception(&mut world, map, Flag::Sensors, true).unwrap();
    assert_eq!(world.btech.maps()[&map].sensor_flags, 224);
    assert!(!world.btech.maps()[&map].perception_disabled(Flag::Sensors));
    assert!(world.btech.maps()[&map].perception_disabled(Flag::Radar));
    assert!(world.btech.maps()[&map].perception_disabled(Flag::Probes));
    let before = world.btech.clone();
    assert!(set_battle_map_perception(&mut world, id, Flag::Sensors, false).is_err());
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.maps()[&map].sensor_flags, 224);
    assert_eq!(
        update_battle_contact(&mut world, id, target, rules)
            .unwrap()
            .transition,
        Transition::Acquired
    );
}

#[tokio::test]
async fn tactical_scanners_use_saved_signatures_and_startup_perception() {
    use stompymux_rs::{
        BattleCharacter, BattleCharacterValue, BattleUnitSignature, battle_contact_observers,
        refresh_battle_contacts, set_battle_character, set_battle_character_value,
        set_battle_unit_signature,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    assert_eq!(
        world.btech.constructed_units()[&id].scanner_perception(),
        18
    );
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 4,
            charisma: 2,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Perception",
        BattleCharacterValue {
            value: 5,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].scanner_perception(),
        18
    );
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Hidden scanner target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 11).unwrap();
    assert!(battle_contact_observers(&world).is_empty());
    let before = world.btech.clone();
    assert!(
        refresh_battle_contacts(&mut world, &[id])
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before);
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(world.btech.constructed_units()[&id].scanner_perception(), 6);
    set_battle_unit_signature(
        &mut world,
        id,
        BattleUnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 2,
            hidden: true,
            illuminated: false,
        },
    )
    .unwrap();
    let observers = battle_contact_observers(&world);
    assert_eq!(observers, vec![id]);
    let before = world.btech.clone();
    assert!(
        refresh_battle_contacts(&mut world, &observers)
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before); // Hidden hostile beyond five hexes: no dice.
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    let events = refresh_battle_contacts(&mut world, &[id, id]).unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].acquired);
    assert!(
        refresh_battle_contacts(&mut world, &observers)
            .unwrap()
            .is_empty()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::InCharacter);
    assert_eq!(battle_contact_observers(&world), vec![id]);
    let before = world.btech.clone();
    assert!(
        refresh_battle_contacts(&mut world, &[id])
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test(flavor = "current_thread")]
async fn automatic_stationary_contact_acquisition_retries_a_failed_save() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture('.').await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let target = world.create(&config, "Automatic contact target".into(), Kind::Thing);
        create_battle_unit(&mut world, target, BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap()).unwrap();
        support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, target, map, 5, 6).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_contact BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'contact failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_contact").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].contacts().contains_key(&target)).await;
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        assert!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].contacts().contains_key(&target));
    }).await;
}

/// The contacts display lists only acquired, currently perceived targets and never rerolls them.
#[tokio::test]
async fn contact_display_filters_unacquired_and_stale_targets_without_rerolls() {
    use stompymux_rs::{
        BattleContactRules, BattleLight, BattleMapPerceptionFlag, set_battle_map_perception,
        set_battle_map_visibility, update_battle_contact,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let mut targets = Vec::new();
    for (name, y) in [("Far contact", 7), ("Near contact", 6), ("Unacquired", 4)] {
        let target = world.create(&config, name.into(), Kind::Thing);
        create_battle_unit(
            &mut world,
            target,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, target, map, 5, y).unwrap();
        targets.push(target);
    }
    let rules = BattleContactRules {
        hostile: false,
        hidden: false,
        perception: 7,
        acquire: true,
    };
    for &target in &targets[..2] {
        update_battle_contact(&mut world, id, target, rules).unwrap();
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "contacts");
    assert!(
        text.lines().filter(|line| line.contains(" x:")).count() == 2,
        "{text}"
    );
    assert!(
        text.find(&format!(
            "[{}]",
            scripts.world().btech.constructed_units()[&targets[0]]
                .battlefield_id()
                .unwrap()
                .to_ascii_lowercase()
        ))
        .unwrap()
            < text
                .find(&format!(
                    "[{}]",
                    scripts.world().btech.constructed_units()[&targets[1]]
                        .battlefield_id()
                        .unwrap()
                        .to_ascii_lowercase()
                ))
                .unwrap()
    );
    assert!(!text.contains(&format!(
            "[{}]",
            scripts.world().btech.constructed_units()[&targets[2]]
                .battlefield_id()
                .unwrap()
                .to_ascii_lowercase()
        )));
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("contacts #{}", targets[0].0),
    );
    assert!(text.lines().filter(|line| line.contains(" x:")).count() == 1);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts/quiet")
            .contains("no switches")
    );
    let first: i64 = scripts.eval_callback(&format!("local c=btech.unit.contacts({}); c[1].target=999999; return btech.unit.contacts({})[1].target", id.0, id.0)).unwrap();
    assert_eq!(first, targets[1].0);
    assert_eq!(scripts.world().btech, before);
    // Without the sensor band or any visibility, saved contacts are stale and hidden.
    set_battle_map_perception(
        &mut scripts.world_mut(),
        map,
        BattleMapPerceptionFlag::Sensors,
        false,
    )
    .unwrap();
    set_battle_map_visibility(&mut scripts.world_mut(), map, BattleLight::Day, 0).unwrap();
    let stale = scripts.world().btech.clone();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts"),
        "Line of Sight Contacts:\r\nEnd Contact List"
    );
    assert_eq!(scripts.world().btech, stale);
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .contacts()
            .len(),
        2
    );
    stop_battle_unit(
        &mut scripts.world_mut(),
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts")
            .contains("Start the unit first")
    );
}

/// Target selection settles across a restart and clears once the contact is lost.
#[tokio::test]
async fn target_selection_settles_after_restart_and_clears_on_contact_loss() {
    use stompymux_rs::{
        BattleLight, BattleMapPerceptionFlag, advance_battle_target_locks, select_battle_target,
        set_battle_map_perception, set_battle_map_visibility,
    };
    let (_dir, config, mut world, id, target) = lock_fixture().await;
    let before = world.btech.clone();
    for (pilot, selected) in [
        (ObjectId(2), target),
        (ObjectId(1), id),
        (ObjectId(1), ObjectId(999999)),
    ] {
        assert!(select_battle_target(&mut world, id, pilot, Some(selected)).is_err());
        assert_eq!(world.btech, before);
    }
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    for _ in 0..3 {
        assert!(advance_battle_target_locks(&mut world).is_empty());
    }
    assert_eq!(
        world.btech.constructed_units()[&id]
            .target_lock()
            .unwrap()
            .remaining,
        5
    );
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    for _ in 0..4 {
        assert!(advance_battle_target_locks(&mut world).is_empty());
    }
    assert_eq!(advance_battle_target_locks(&mut world).len(), 1);
    assert_eq!(
        world.btech.constructed_units()[&id]
            .target_lock()
            .unwrap()
            .remaining,
        0
    );
    assert!(advance_battle_target_locks(&mut world).is_empty());
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    // A read-only visibility check does not mutate selection or reroll acquisition.
    assert!(
        stompymux_rs::visible_battle_contacts(&world, id)
            .unwrap()
            .is_empty()
    );
    for _ in 0..8 {
        assert!(advance_battle_target_locks(&mut world).is_empty());
    }
    assert_eq!(
        world.btech.constructed_units()[&id]
            .target_lock()
            .unwrap()
            .remaining,
        0
    );
    let events = stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].lock_lost);
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
    assert!(world.btech.constructed_units()[&id].contacts().is_empty());
    assert!(select_battle_target(&mut world, id, ObjectId(1), Some(target)).is_err());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, true).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    place_battle_unit(&mut world, target, map, 5, 7).unwrap();
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
}

/// Light changes leave a lock alone; explicit clearing and shutdown remove it.
#[tokio::test]
async fn target_selection_survives_light_changes_and_clears_on_shutdown_and_explicit_clear() {
    use stompymux_rs::{BattleLight, select_battle_target, set_battle_map_visibility};
    let (_dir, config, mut world, id, target) = lock_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let selected = world.btech.constructed_units()[&id].target_lock();
    assert!(selected.is_some());
    for light in [BattleLight::Night, BattleLight::Twilight, BattleLight::Day] {
        set_battle_map_visibility(&mut world, map, light, 30).unwrap();
        assert_eq!(world.btech.constructed_units()[&id].target_lock(), selected);
    }
    assert!(
        stompymux_rs::refresh_battle_contacts(&mut world, &[id])
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech.constructed_units()[&id].target_lock(), selected);
    select_battle_target(&mut world, id, ObjectId(1), None).unwrap();
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn target_selection_command_and_lua_share_guards_and_rollback() {
    let (_dir, config, world, id, target) = lock_fixture().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "lock/nope").contains("no switches")
    );
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "lock bad").contains("Usage"));
    assert_eq!(scripts.world().btech, before);
    support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("lock #{}", target.0),
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .target_lock()
            .unwrap()
            .target,
        target
    );
    let remaining: u8 = scripts
        .eval_callback(&format!(
            "return btech.unit.state({}).target_lock.remaining",
            id.0
        ))
        .unwrap();
    assert_eq!(remaining, 8);
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.lock({}, 1, nil); error('rollback')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    scripts
        .eval_callback::<()>(&format!("btech.unit.lock({}, 1, nil)", id.0))
        .unwrap();
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .target_lock()
            .is_none()
    );
    support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("lock #{}", target.0),
    );
    support::run_text(&scripts, &config, ObjectId(1), 1, "lock -");
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .target_lock()
            .is_none()
    );
}

#[tokio::test]
async fn target_lock_server_completion_retries_failed_commit() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id, target) = lock_fixture().await;
        stompymux_rs::select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
        for _ in 0..7 { stompymux_rs::advance_battle_target_locks(&mut world); }
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // The lock settling removes a timer row; refuse the commit at the snapshot stamp.
        sqlx::query("CREATE TRIGGER deny_lock BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'lock failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_lock").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].target_lock().unwrap().remaining == 0).await;
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn target_lock_rejects_corrupt_state_and_clears_on_destruction() {
    let (_dir, config, mut world, id, target) = lock_fixture().await;
    stompymux_rs::select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    for (selected, remaining) in [(id.0, 8), (999999, 8), (target.0, 9)] {
        let mut corrupt = world.clone();
        corrupt
            .btech
            .rewrite_unit_record(id, |record| {
                record["target_lock"] =
                    serde_json::json!({"target":selected,"remaining":remaining});
            })
            .unwrap();
        assert!(corrupt.validate(&config).is_err());
    }
    let cockpit = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .find(|system| system.system == stompymux_rs::BattleSystem::Cockpit)
        .unwrap();
    stompymux_rs::destroy_battle_critical(&mut world, id, cockpit.location).unwrap();
    assert!(world.btech.constructed_units()[&id].is_destroyed());
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn aim_lock_penalty_follows_selected_target_and_committed_settling() {
    use stompymux_rs::{advance_battle_target_locks, battle_aim_modifiers, select_battle_target};
    let (_dir, config, mut world, id, rear) = lock_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let front = world.create(&config, "Forward target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        front,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, front, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, front, map, 5, 4).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    let penalty = |world: &stompymux_rs::World, target| {
        battle_aim_modifiers(world, id, target, 0, 4, optical_aim_rules())
            .unwrap()
            .target_lock
    };
    assert_eq!(penalty(&world, front), 2); // No selection, even though the shot is forward.
    select_battle_target(&mut world, id, ObjectId(1), Some(rear)).unwrap();
    assert_eq!(penalty(&world, rear), 2);
    assert_eq!(penalty(&world, front), 2); // Arc belongs to the selected rear target.
    for _ in 0..8 {
        advance_battle_target_locks(&mut world);
    }
    assert_eq!(penalty(&world, rear), 0);
    assert_eq!(penalty(&world, front), 2);
    select_battle_target(&mut world, id, ObjectId(1), Some(front)).unwrap();
    assert_eq!(penalty(&world, front), 1);
    assert_eq!(penalty(&world, rear), 1);
    stompymux_rs::rotate_battle_torso(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleTorso::Right,
    )
    .unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 30.0).unwrap();
    for _ in 0..30 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert_eq!(penalty(&world, front), 2); // Torso and heading together put selection to the side.
    let mut rules = optical_aim_rules();
    rules.override_weapon_arcs = true;
    assert_eq!(
        battle_aim_modifiers(&world, id, rear, 0, 4, rules)
            .unwrap()
            .target_lock,
        0
    );
    let before = world.btech.clone();
    let expected = battle_aim_modifiers(&world, id, front, 0, 4, optical_aim_rules()).unwrap();
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_aim_modifiers(&loaded, id, front, 0, 4, optical_aim_rules()).unwrap(),
        expected
    );
}

/// Aim takes the best perceiving channel's modifier and never uses stale contacts.
#[tokio::test]
async fn perception_aim_chooses_the_best_channel_and_never_uses_stale_contacts() {
    use stompymux_rs::{
        BattleDetectionChannel as Channel, BattleLight, BattleMapPerceptionFlag,
        BattlePerceptionAim, battle_aim_modifiers, set_battle_map_perception,
        set_battle_map_visibility,
    };
    let (_dir, _config, mut world, id, target) = lock_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    let aim = |world: &stompymux_rs::World| {
        battle_aim_modifiers(world, id, target, 0, 4, optical_aim_rules()).unwrap()
    };
    let perceived = |channel, modifier| {
        Some(BattlePerceptionAim {
            channel: Some(channel),
            direct_fire: true,
            modifier,
        })
    };
    // Darkness does not matter inside the sensor band.
    assert_eq!(aim(&world).perception, perceived(Channel::Sensors, 0));
    // Sight alone pays +1 at night for an unlit target.
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    assert_eq!(aim(&world).perception, perceived(Channel::Sight, 1));
    set_battle_map_visibility(&mut world, map, BattleLight::Twilight, 30).unwrap();
    assert_eq!(aim(&world).perception, perceived(Channel::Sight, 0));
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    stompymux_rs::set_battle_unit_signature(
        &mut world,
        target,
        stompymux_rs::BattleUnitSignature {
            team: 0,
            hidden: false,
            illuminated: true,
        },
    )
    .unwrap();
    assert_eq!(aim(&world).perception, perceived(Channel::Sight, 0));
    // With nothing reaching the target, the saved contact is not used.
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    let before = world.btech.clone();
    assert!(aim(&world).perception.is_none());
    assert!(aim(&world).subtotal().is_none());
    assert_eq!(world.btech, before); // Reading stale saved observations cannot reroll them.
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, true).unwrap();
    assert_eq!(aim(&world).perception, perceived(Channel::Sensors, 0));
    place_battle_unit(&mut world, target, map, 5, 6).unwrap(); // Clears acquisition, despite geometric visibility.
    assert!(aim(&world).perception.is_none());
    assert!(aim(&world).subtotal().is_none());
}

/// Perception aim includes woods in the target hex and partial cover from shallow water.
#[tokio::test]
async fn perception_aim_includes_target_woods_and_shallow_water_cover() {
    for (tile, target_y, expected) in [("`0", 6, 1), ("~1", 6, 0), ("~1", 7, 3)] {
        let source = format!(
            "12 12\n{}",
            (0..12)
                .map(|y| {
                    (0..12)
                        .map(|x| if (x, y) == (5, target_y) { tile } else { ".0" })
                        .collect::<String>()
                        + "\n"
                })
                .collect::<String>()
        );
        let (_dir, config, mut world, id) = fixture_source(&source).await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let target = world.create(&config, "Covered target".into(), Kind::Thing);
        create_battle_unit(
            &mut world,
            target,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, target, map, 5, target_y).unwrap();
        stompymux_rs::set_battle_map_visibility(
            &mut world,
            map,
            stompymux_rs::BattleLight::Night,
            30,
        )
        .unwrap();
        stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
        let terrain = stompymux_rs::battle_unit_terrain_los(&world, id, target).unwrap();
        // Sensor-band cover: path woods, target woods and three for partial cover.
        assert_eq!(
            i16::from(terrain.woods)
                + i16::from(terrain.target_woods)
                + if terrain.partial_cover { 3 } else { 0 },
            expected,
            "{tile}"
        );
        let aim = stompymux_rs::battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules())
            .unwrap();
        let perception = aim.perception.unwrap();
        assert_eq!(
            perception.channel,
            Some(stompymux_rs::BattleDetectionChannel::Sensors)
        );
        assert_eq!(perception.modifier, expected, "{tile}");
        // Gunnery 4 + off target -4 + no lock 2 + perception contribution.
        assert_eq!(aim.subtotal(), Some(2 + i32::from(expected)));
    }
}

#[tokio::test]
async fn pilot_gunnery_uses_current_family_skills_connection_and_signed_targets() {
    use stompymux_rs::{
        BattleCharacter, BattleCharacterValue, BattleWeapon as W, Flag, battle_gunnery_target,
        battle_pilot_aim_modifiers, battle_unit_gunnery_target, set_battle_character,
        set_battle_character_value,
    };
    let (_dir, config, mut world, id, target) = lock_fixture().await;
    let pilot = ObjectId(1);
    let read =
        |world: &stompymux_rs::World| battle_unit_gunnery_target(world, id, 0, true).unwrap();
    assert_eq!(read(&world), 6); // Assigned but disconnected.
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(read(&world), 18); // Connected with no character record.
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assert_eq!(read(&world), 11);
    for (name, value) in [
        ("Gunnery-Laser", 5),
        ("Gunnery-Missile", 6),
        ("Gunnery-Ballistic", 7),
        ("Gunnery-Battlemech", 8),
    ] {
        set_battle_character_value(
            &mut world,
            pilot,
            name,
            BattleCharacterValue {
                value,
                experience: 16_777_216 + 123,
                last_used: 42,
            },
        )
        .unwrap();
    }
    for (weapon, skill) in [
        (W::MediumLaser, 5),
        (W::Srm4, 6),
        (W::Srm6, 6),
        (W::Lrm20, 6),
        (W::Ac20, 7),
    ] {
        assert_eq!(
            battle_gunnery_target(&world, pilot, weapon, true).unwrap(),
            10 - skill
        );
        assert_eq!(
            battle_gunnery_target(&world, pilot, weapon, false).unwrap(),
            2
        );
    }
    assert_eq!(read(&world), 5);
    let aimed =
        battle_pilot_aim_modifiers(&world, id, target, 0, true, optical_aim_rules()).unwrap();
    assert_eq!(aimed.gunnery, 5);
    let before = world.btech.clone();
    assert!(battle_unit_gunnery_target(&world, id, 99999, true).is_err());
    assert!(battle_gunnery_target(&world, id, W::MediumLaser, true).is_err());
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_gunnery_target(&loaded, pilot, W::MediumLaser, true).unwrap(),
        5
    );
    // Skills are read at query time; changing them does not require an engine restart.
    set_battle_character_value(
        &mut world,
        pilot,
        "Gunnery-Laser",
        BattleCharacterValue {
            value: 15,
            experience: 0,
            last_used: 77,
        },
    )
    .unwrap();
    let aim = battle_pilot_aim_modifiers(&world, id, target, 0, true, optical_aim_rules()).unwrap();
    assert_eq!(aim.gunnery, -4);
    assert_eq!(aim.subtotal(), Some(-6)); // -4 gunnery -4 immobile target +2 unlocked.
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Connected);
    assert_eq!(read(&world), 6);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let default: i16 = scripts
        .eval_callback(&format!("return btech.unit.gunnery({}, 0)", id.0))
        .unwrap();
    assert_eq!(default, 6);
    scripts
        .world_mut()
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let skill: i16 = scripts
        .eval_callback(&format!("return btech.unit.gunnery({}, 0)", id.0))
        .unwrap();
    assert_eq!(
        skill,
        if config.battletech.extended_gunnery != 0 {
            -4
        } else {
            2
        }
    );
    assert!(
        scripts
            .eval_callback::<i16>(&format!("return btech.unit.gunnery({}, -1)", id.0))
            .is_err()
    );
}

#[tokio::test]
async fn signed_aim_subtotal_does_not_overflow_at_supplied_skill_limits() {
    let (_dir, _config, world, id, target) = lock_fixture().await;
    for gunnery in [i16::MIN, i16::MAX] {
        let aim =
            stompymux_rs::battle_aim_modifiers(&world, id, target, 0, gunnery, optical_aim_rules())
                .unwrap();
        assert_eq!(aim.subtotal(), Some(i32::from(gunnery) - 2));
    }
}

#[tokio::test]
async fn direct_shot_hit_threshold_and_expenditure_match_manual_composition() {
    use stompymux_rs::{
        BattleHitArc, resolve_battle_shot, resolve_battle_tactical_salvo, roll_unit_dice,
        spend_battle_weapon,
    };
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let (seed, roll) = (0..=255)
        .map(|seed| (seed, stompymux_rs::BattleDice::seeded([seed; 32]).two_d6()))
        .find(|(_, roll)| *roll <= 8)
        .unwrap();
    shot_seed(&mut base, id, seed);
    shot_seed(&mut base, target, 17);
    for (level, hit) in [(9 - roll, true), (8 - roll, false)] {
        let mut world = base.clone();
        shot_skill(&mut world, level);
        let mut expected = world.clone();
        let expenditure = spend_battle_weapon(&mut expected, id, ObjectId(1), 0).unwrap();
        assert_eq!(
            roll_unit_dice(&mut expected, id, 2)
                .unwrap()
                .into_iter()
                .sum::<u8>(),
            roll
        );
        let salvo = hit.then(|| {
            resolve_battle_tactical_salvo(
                &mut expected,
                target,
                expenditure.weapon,
                BattleHitArc::Rear,
                fall_rules(),
            )
            .unwrap()
        });
        if !hit {
            expected_grass_miss_rolls(|| {
                roll_unit_dice(&mut expected, id, 2).unwrap().iter().sum()
            });
        }
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).unwrap();
        assert_eq!(report.roll, roll);
        assert_eq!(
            report.target_number,
            Some(i32::from(roll) + i32::from(!hit))
        );
        assert_eq!(report.expenditure, expenditure);
        assert_eq!(
            report.salvo,
            salvo.map(stompymux_rs::BattleTargetSalvo::Mech)
        );
        // Ammo/damage primitives do not represent a launched, targeted attack on their own.
        let mut state = serde_json::to_value(&expected.btech).unwrap();
        state["constructed"][id.0.to_string()]["shot_counters"] =
            serde_json::json!({"fired": 1, "hit": i32::from(hit), "missed": i32::from(!hit)});
        // The seeded material cascade includes its own ammunition explosion.
        // Attribute all independently resolved target packets to the actual shooter.
        let inflicted =
            state["constructed"][target.0.to_string()]["damage_counters"]["taken"].clone();
        // Five laser damage ignites the seeded 200-point ammunition packet.
        // Keep a numeric oracle as well as the independently composed cascade:
        // omitting explosion accounting from both paths must still fail.
        assert_eq!(inflicted, if hit { 205 } else { 0 });
        state["constructed"][id.0.to_string()]["damage_counters"] = serde_json::json!({
            "taken": 0, "inflicted": inflicted
        });
        // The nested ammunition explosion destroys this fixture once.
        state["constructed"][id.0.to_string()]["units_killed"] = i32::from(hit).into();
        expected.btech = serde_json::from_value(state).unwrap();
        assert_eq!(world.btech, expected.btech);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Rejected direct shots leave both dice streams and all expenditure untouched.
#[tokio::test]
async fn direct_shot_rejections_preserve_both_dice_streams_and_all_expenditure() {
    use stompymux_rs::resolve_battle_shot;
    let (_dir, _config, mut world, id, target) = shot_fixture().await;
    let before = world.btech.clone();
    for (pilot, selected, index) in [
        (ObjectId(2), target, 0),
        (ObjectId(1), id, 0),
        (ObjectId(1), target, 9999),
        (ObjectId(1), ObjectId(99999), 0),
    ] {
        assert!(resolve_battle_shot(&mut world, id, pilot, selected, index, shot_rules()).is_err());
        assert_eq!(world.btech, before);
    }
    let mut rules = shot_rules();
    rules.hit_arc_mode = 3;
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).is_err());
    assert_eq!(world.btech, before);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    // With neither the sensor band nor sight reaching the target, the contact is not perceived.
    stompymux_rs::set_battle_map_perception(
        &mut world,
        map,
        stompymux_rs::BattleMapPerceptionFlag::Sensors,
        false,
    )
    .unwrap();
    stompymux_rs::set_battle_map_visibility(&mut world, map, stompymux_rs::BattleLight::Day, 0)
        .unwrap();
    let before = world.btech.clone();
    let refused = format!(
        "{:#}",
        resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).unwrap_err()
    );
    assert!(
        refused.contains("Target is not a current acquired contact"),
        "{refused}"
    );
    assert_eq!(world.btech, before);
    stompymux_rs::set_battle_map_perception(
        &mut world,
        map,
        stompymux_rs::BattleMapPerceptionFlag::Sensors,
        true,
    )
    .unwrap();
    stompymux_rs::set_battle_map_visibility(&mut world, map, stompymux_rs::BattleLight::Day, 30)
        .unwrap();
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    let before = world.btech.clone();
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).is_err());
    assert_eq!(world.btech, before);
    rules = shot_rules();
    rules.aim.override_weapon_arcs = true;
    let report = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
    assert!(report.salvo.is_none()); // Connected, no character: target 14 with override.
    let before = world.btech.clone();
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).is_err());
    assert_eq!(world.btech, before); // Recycling cannot consume another shot.
}

#[tokio::test]
async fn direct_shot_partial_cover_uses_one_upper_body_die_per_group() {
    let source = format!(
        "12 12\n{}",
        (0..12)
            .map(|y| (0..12)
                .map(|x| if (x, y) == (5, 3) { "~1" } else { ".0" })
                .collect::<String>()
                + "\n")
            .collect::<String>()
    );
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Covered shot target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 3).unwrap();
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    shot_skill(&mut world, 20);
    let mut dice = stompymux_rs::BattleDice::seeded([19; 32]);
    let location = stompymux_rs::BattleHitTable::Punch
        .location(
            stompymux_rs::BattleMechChassis::Biped,
            stompymux_rs::BattleHitArc::Rear,
            dice.d6(),
        )
        .unwrap();
    shot_seed(&mut world, target, 19);
    let report =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules())
            .unwrap();
    assert_eq!(report.aim.perception.unwrap().modifier, 3);
    let salvo = report.salvo.unwrap().into_mech().unwrap();
    assert_eq!(salvo.groups.len(), 1);
    assert_eq!(salvo.groups[0].hit.section, location);
    assert!(!salvo.groups[0].hit.through_armor_critical);
    assert!(!salvo.groups[0].hit.crew_stun);
    assert!(!matches!(
        location,
        stompymux_rs::BattleSection::LeftLeg | stompymux_rs::BattleSection::RightLeg
    ));
}

#[tokio::test]
async fn direct_missile_shot_spends_one_salvo_and_groups_damage_atomically() {
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 20);
    shot_seed(&mut world, target, 29);
    stompymux_rs::set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Gunnery-Missile",
        stompymux_rs::BattleCharacterValue {
            value: 20,
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
        .position(|mount| mount.weapon == stompymux_rs::BattleWeapon::Srm4)
        .unwrap();
    let before = world.btech.constructed_units()[&id].clone();
    let report =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
            .unwrap();
    let after = &world.btech.constructed_units()[&id];
    let bin = report.expenditure.ammunition[0].bin_index;
    assert_eq!(after.ammunition()[bin], before.ammunition()[bin] - 1);
    assert_eq!(
        after.heat().stored,
        before.heat().stored + f64::from(report.expenditure.heat)
    );
    assert_eq!(after.weapon_recycle()[&index], 15);
    let salvo = report.salvo.unwrap().into_mech().unwrap();
    assert!(salvo.cluster_roll.is_some());
    assert!(salvo.groups.iter().all(|group| group.damage == 2));
    assert_eq!(
        salvo.groups.len(),
        usize::from(
            stompymux_rs::BattleWeapon::Srm4
                .missile_hits(salvo.cluster_roll.unwrap())
                .unwrap()
        )
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn direct_out_of_range_shot_still_rolls_and_spends_without_target_damage() {
    let row = ".0".repeat(12) + "\n";
    let source = format!("12 30\n{}", row.repeat(30));
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Distant shot target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 20).unwrap();
    shot_seed(&mut world, id, 23);
    for _ in 0..100 {
        stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
        if world.btech.constructed_units()[&id]
            .contacts()
            .contains_key(&target)
        {
            break;
        }
    }
    assert!(
        world.btech.constructed_units()[&id]
            .contacts()
            .contains_key(&target)
    );
    let target_before = world.btech.constructed_units()[&target].clone();
    let mut rules = shot_rules();
    rules.aim.override_weapon_arcs = true;
    rules.glancing = stompymux_rs::BattleGlancingMode::BelowTarget;
    let report =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
    assert!(report.target_number.is_none());
    assert!(report.salvo.is_none());
    assert!(!report.glancing);
    assert_eq!(world.btech.constructed_units()[&target], target_before);
    assert_eq!(
        world.btech.constructed_units()[&id].weapon_recycle()[&0],
        20
    );
    assert_eq!(world.btech.constructed_units()[&id].heat().stored, 3.0);
    // An out-of-range Streak attempt recycles but leaves the prior heat and ammunition untouched.
    let index = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == stompymux_rs::BattleWeapon::Srm4)
        .unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for part in definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
    {
        part.equipment = part.equipment.replace("IS.SRM-4", "IS.StreakSRM-4");
    }
    world.btech.set_unit_definition(id, definition).unwrap();
    let report =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, index, rules)
            .unwrap();
    assert!(!report.launched);
    assert!(report.target_number.is_none());
    assert!(report.salvo.is_none());
    assert_eq!(report.expenditure.heat, 0);
    assert!(report.expenditure.ammunition.is_empty());
    assert_eq!(world.btech.constructed_units()[&id].heat().stored, 3.0);
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[25]);
    assert_eq!(
        world.btech.constructed_units()[&id].weapon_recycle()[&index],
        15
    );
    assert_eq!(world.btech.constructed_units()[&target], target_before);
}

#[tokio::test]
async fn direct_shot_failed_save_rolls_back_both_units_and_replays_identically() {
    use sqlx::Connection;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 20);
    shot_seed(&mut world, id, 31);
    shot_seed(&mut world, target, 32);
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.clone();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER deny_shot BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'target shot failure'); END", target.0))).execute(&mut sql).await.unwrap();
    let first =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules())
            .unwrap();
    assert!(first.salvo.is_some());
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before.btech
    );
    world = before;
    sqlx::query("DROP TRIGGER deny_shot")
        .execute(&mut sql)
        .await
        .unwrap();
    let retry =
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules())
            .unwrap();
    assert_eq!(retry, first);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn piloting_checks_use_saved_skills_damage_and_replayable_dice() {
    use stompymux_rs::{
        BattleCharacterValue, BattleSection, CriticalLocation, Flag, battle_unit_piloting_target,
        roll_battle_piloting,
    };
    let (_dir, config, mut world, id) = fixture('.').await;
    assert_eq!(battle_unit_piloting_target(&world, id, true).unwrap(), 6);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(battle_unit_piloting_target(&world, id, true).unwrap(), 18);
    shot_skill(&mut world, 0); // Supplies reflexes 4 and intuition 3.
    for (name, value) in [("Piloting-Biped", 4), ("Piloting-Battlemech", 5)] {
        stompymux_rs::set_battle_character_value(
            &mut world,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value,
                experience: 16_777_216 + 20,
                last_used: 123,
            },
        )
        .unwrap();
    }
    assert_eq!(battle_unit_piloting_target(&world, id, true).unwrap(), 6);
    assert_eq!(battle_unit_piloting_target(&world, id, false).unwrap(), 5);
    stompymux_rs::destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::LeftLeg,
            slot: 0,
        },
    )
    .unwrap();
    let gyro = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .find(|system| system.system == stompymux_rs::BattleSystem::Gyro)
        .unwrap();
    stompymux_rs::destroy_battle_critical(&mut world, id, gyro.location).unwrap();
    shot_seed(&mut world, id, 12);
    persistence::save(&config.database(), &world).await.unwrap();
    let expected_roll = stompymux_rs::BattleDice::seeded([12; 32]).two_d6();
    let check = roll_battle_piloting(&mut world, id, -1, true).unwrap();
    assert_eq!(check.skill, 6);
    assert_eq!(check.damage, 5); // One hip + one gyro.
    assert_eq!(check.target, 10);
    assert_eq!(check.roll, Some(expected_roll));
    assert_eq!(check.success, expected_roll >= 10);
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(
        roll_battle_piloting(&mut loaded, id, -1, true).unwrap(),
        check
    );
    assert_eq!(loaded.btech, world.btech);
    // Equality succeeds and signed modifiers cannot overflow target arithmetic.
    let mut expected_dice = stompymux_rs::BattleDice::seeded([14; 32]);
    let roll = expected_dice.two_d6();
    shot_seed(&mut world, id, 14);
    let exact = roll_battle_piloting(&mut world, id, i16::from(roll) - 11, true).unwrap();
    assert!(exact.success);
    assert_eq!(exact.target, i32::from(roll));
    assert!(
        roll_battle_piloting(&mut world, id, i16::MIN, true)
            .unwrap()
            .success
    );
    assert!(
        !roll_battle_piloting(&mut world, id, i16::MAX, true)
            .unwrap()
            .success
    );
}

#[tokio::test]
async fn stopped_and_unconscious_piloting_checks_fail_without_consuming_dice() {
    use stompymux_rs::roll_battle_piloting;
    let (_dir, _config, world, id) = fixture('.').await;
    let mut stopped = world.clone();
    stop_battle_unit(
        &mut stopped,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before = stopped.btech.clone();
    let check = roll_battle_piloting(&mut stopped, id, -100, true).unwrap();
    assert!(!check.success);
    assert!(check.roll.is_none());
    assert_eq!(stopped.btech, before);
    let mut world = world;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 10)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"]["1"] = serde_json::json!({
        "mode": {"kind":"tactical", "injuries":0}, "remaining":0,
        "pain_resistance":false, "toughness":false,
        "dice":stompymux_rs::BattleDice::seeded([seed;32])
    });
    world.btech = serde_json::from_value(state).unwrap();
    stompymux_rs::injure_battle_tactical_pilot(&mut world, id, 4, false).unwrap();
    assert!(world.btech.unconscious(ObjectId(1)));
    let before = world.btech.clone();
    let check = roll_battle_piloting(&mut world, id, -100, false).unwrap();
    assert!(!check.success);
    assert!(check.roll.is_none());
    assert_eq!(world.btech, before);
    assert!(roll_battle_piloting(&mut world, ObjectId(99999), 0, false).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn piloting_check_missing_character_pilot_adds_five_but_stun_still_rolls() {
    let (_dir, _config, mut world, id) = fixture('.').await;
    stompymux_rs::stun_battle_unit(&mut world, id).unwrap();
    let check = stompymux_rs::roll_battle_piloting(&mut world, id, 0, true).unwrap();
    assert!(check.roll.is_some());
    assert_eq!(check.absent_character_pilot, 0);
    stompymux_rs::release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::InCharacter);
    let check = stompymux_rs::roll_battle_piloting(&mut world, id, 0, true).unwrap();
    assert_eq!(check.skill, 6);
    assert_eq!(check.absent_character_pilot, 5);
    assert_eq!(check.target, 11);
    assert!(check.roll.is_some());
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Going);
    let before = world.btech.clone();
    assert!(stompymux_rs::roll_battle_piloting(&mut world, id, 0, true).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn dry_fall_stops_motion_rotates_resets_facing_and_replays_after_restart() {
    use stompymux_rs::{BattleHitArc, BattlePosture, resolve_battle_fall};
    let (_dir, config, mut world, id) = fixture('.').await;
    set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    stompymux_rs::rotate_battle_torso(&mut world, id, ObjectId(1), stompymux_rs::BattleTorso::Left)
        .unwrap();
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() >= 8)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let old = world.btech.constructed_units()[&id].motion().unwrap();
    let mut dice = stompymux_rs::BattleDice::seeded([seed; 32]);
    let avoid = dice.two_d6();
    let direction = dice.d6();
    persistence::save(&config.database(), &world).await.unwrap();
    let report = resolve_battle_fall(&mut world, id, 2, fall_rules()).unwrap();
    assert_eq!(report.avoidance.unwrap().roll, Some(avoid));
    assert!(report.avoidance.unwrap().success);
    assert!(report.pilot_injury.is_none());
    assert_eq!(report.direction_roll, direction);
    assert_eq!(
        report.arc,
        match direction {
            1 => BattleHitArc::Front,
            2 | 3 => BattleHitArc::Right,
            4 => BattleHitArc::Rear,
            _ => BattleHitArc::Left,
        }
    );
    assert_eq!(report.damage, 8);
    assert_eq!(
        report.groups.iter().map(|g| g.damage).collect::<Vec<_>>(),
        [5, 3]
    );
    let fallen = &world.btech.constructed_units()[&id];
    assert_eq!(fallen.posture(), BattlePosture::Prone);
    assert_eq!(fallen.facing(), stompymux_rs::BattleFacing::default());
    let motion = fallen.motion().unwrap();
    assert_eq!(motion.point, old.point);
    assert_eq!((motion.speed, motion.desired_speed), (0.0, 0.0));
    assert_eq!(
        motion.heading,
        (old.heading + f64::from(direction - 1) * 60.0).rem_euclid(360.0)
    );
    assert_eq!(motion.heading, motion.desired_heading);
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_battle_fall(&mut loaded, id, 2, fall_rules()).unwrap(),
        report
    );
    assert_eq!(loaded.btech, world.btech);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let before = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 10.0).is_err());
    assert!(
        stompymux_rs::rotate_battle_torso(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleTorso::Right
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let check = stompymux_rs::roll_battle_piloting(&mut world, id, 100, true).unwrap();
    assert!(check.success);
    assert!(check.roll.is_none());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn dry_fall_injury_and_invalid_requests_preserve_atomicity() {
    let (_dir, _config, mut world, id) = fixture('.').await;
    let before = world.btech.clone();
    assert!(stompymux_rs::resolve_battle_fall(&mut world, id, 0, fall_rules()).is_err());
    assert_eq!(world.btech, before);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 7)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let report = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    assert!(!report.avoidance.unwrap().success);
    assert_eq!(report.pilot_injury.unwrap().injuries, 1);
    assert!(world.btech.constructed_units()[&id].pilot_injuries() >= 1);
    let (_dir, _config, mut bridge, id) = fixture('/').await;
    let fall = stompymux_rs::resolve_battle_fall(&mut bridge, id, 1, fall_rules()).unwrap();
    assert_eq!(fall.damage, 4);
    assert_eq!(
        bridge.btech.constructed_units()[&id].posture(),
        stompymux_rs::BattlePosture::Prone
    );
}

#[tokio::test]
async fn prone_posture_changes_los_and_aim_at_adjacent_and_distant_ranges() {
    let source = format!(
        "12 12\n{}",
        (0..12)
            .map(|y| (0..12)
                .map(|x| if (x, y) == (5, 4) { ".1" } else { ".0" })
                .collect::<String>()
                + "\n")
            .collect::<String>()
    );
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Prone target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 3).unwrap();
    let standing = stompymux_rs::battle_unit_terrain_los(&world, id, target).unwrap();
    assert!(!standing.blocked);
    assert!(standing.partial_cover);
    shot_seed(&mut world, target, 17);
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, target, 1, fall_rules()).unwrap();
    let prone = stompymux_rs::battle_unit_terrain_los(&world, id, target).unwrap();
    assert!(prone.blocked);
    assert!(!prone.partial_cover);
    assert_eq!(
        stompymux_rs::battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules())
            .unwrap()
            .target_movement,
        -3
    );
    place_battle_unit(&mut world, target, map, 5, 6).unwrap();
    assert_eq!(
        stompymux_rs::battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules())
            .unwrap()
            .target_movement,
        -6
    );
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].attacker_movement_modifier(false),
        2
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let posture: String = scripts
        .eval_callback(&format!("return btech.unit.state({}).posture", id.0))
        .unwrap();
    assert_eq!(posture, "prone");
}

#[tokio::test]
async fn fall_gravity_only_reduces_damage_under_special_map_rules() {
    let (_dir, _config, base, id) = fixture('.').await;
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    for (flags, gravity, expected) in [(0, 50, 4), (2, 50, 2), (2, 200, 4)] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_map_record(map, |record| {
                record["flags"] = serde_json::json!(flags);
                record["gravity"] = serde_json::json!(gravity);
            })
            .unwrap();
        shot_seed(&mut world, id, 17);
        let fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
        assert_eq!(fall.damage, expected);
    }
}

#[tokio::test]
async fn successful_stand_is_upright_immediately_and_finishes_after_durable_delay() {
    use stompymux_rs::{
        BattlePosture, BattleStandMode as M, BattleStandTimer as T, advance_battle_standing,
        begin_battle_stand,
    };
    let (_dir, config, mut world, id) = stand_fixture().await;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() >= 6)
        .unwrap();
    shot_seed(&mut world, id, seed);
    assert_eq!(
        stompymux_rs::battle_stand_target(&world, id, ObjectId(1), true).unwrap(),
        6
    );
    let attempt =
        begin_battle_stand(&mut world, id, ObjectId(1), M::Normal, true, fall_rules()).unwrap();
    assert!(attempt.check.success);
    assert!(attempt.fall.is_none());
    assert_eq!(attempt.timer, Some(T::Rising { remaining: 5 }));
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Standing
    );
    assert_eq!(
        world.btech.constructed_units()[&id].attacker_movement_modifier(false),
        2
    );
    let before = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 10.0).is_err());
    assert!(
        begin_battle_stand(&mut world, id, ObjectId(1), M::Normal, true, fall_rules()).is_err()
    );
    assert_eq!(world.btech, before);
    for _ in 0..2 {
        assert!(advance_battle_standing(&mut world).is_empty());
    }
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    for _ in 0..2 {
        assert!(advance_battle_standing(&mut world).is_empty());
    }
    assert_eq!(advance_battle_standing(&mut world).len(), 1);
    assert!(world.btech.constructed_units()[&id].stand_timer().is_none());
    assert!(advance_battle_standing(&mut world).is_empty());
    set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn careful_stand_failure_falls_again_and_requires_thirty_second_recovery() {
    use stompymux_rs::{
        BattlePosture, BattleStandMode as M, BattleStandTimer as T, advance_battle_standing,
        begin_battle_stand,
    };
    let (_dir, _config, mut world, id) = stand_fixture().await;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 4)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let before = world.btech.clone();
    assert!(
        begin_battle_stand(&mut world, id, ObjectId(1), M::Careful, false, fall_rules()).is_err()
    );
    assert_eq!(world.btech, before);
    let attempt =
        begin_battle_stand(&mut world, id, ObjectId(1), M::Careful, true, fall_rules()).unwrap();
    assert!(!attempt.check.success);
    assert_eq!(attempt.check.target, 4);
    let fall = attempt.fall.unwrap();
    assert_eq!(fall.damage, 4);
    assert!(fall.avoidance.unwrap().roll.is_some()); // Attempt clears prone before checking/falling.
    assert_eq!(attempt.timer, Some(T::Recovering { remaining: 30 }));
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    let before = world.btech.clone();
    assert!(
        begin_battle_stand(&mut world, id, ObjectId(1), M::Anyway, true, fall_rules()).is_err()
    );
    assert_eq!(world.btech, before);
    for _ in 0..29 {
        assert!(advance_battle_standing(&mut world).is_empty());
    }
    assert_eq!(advance_battle_standing(&mut world).len(), 1);
    assert!(world.btech.constructed_units()[&id].stand_timer().is_none());
}

#[tokio::test]
async fn stand_command_inspection_and_impossible_target_guards_are_atomic() {
    let (_dir, config, mut world, id) = stand_fixture().await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "stand check")
            .contains("BTH to stand would be: 6")
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "stand/nope").contains("no switches")
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "stand unknown").contains("Usage")
    );
    assert_eq!(scripts.world().btech, before);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() >= 6)
        .unwrap();
    shot_seed(&mut scripts.world_mut(), id, seed);
    support::run_text(&scripts, &config, ObjectId(1), 1, "stand careful");
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].stand_timer(),
        Some(stompymux_rs::BattleStandTimer::Rising { remaining: 10 })
    );
    let state: String = scripts
        .eval_callback(&format!(
            "return btech.unit.state({}).stand_timer.state",
            id.0
        ))
        .unwrap();
    assert_eq!(state, "rising");
    stompymux_rs::set_battle_character(
        &mut world,
        ObjectId(1),
        stompymux_rs::BattleCharacter {
            build: 5,
            reflexes: 0,
            intuition: 0,
            learn: 0,
            charisma: 0,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let before = world.btech.clone(); // Target 13; careful must still refuse an impossible unmodified target.
    for mode in [
        stompymux_rs::BattleStandMode::Normal,
        stompymux_rs::BattleStandMode::Careful,
    ] {
        assert!(
            stompymux_rs::begin_battle_stand(&mut world, id, ObjectId(1), mode, true, fall_rules())
                .is_err()
        );
        assert_eq!(world.btech, before);
    }
    let attempt = stompymux_rs::begin_battle_stand(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleStandMode::Anyway,
        true,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(attempt.check.target, 13);
    assert!(!attempt.check.success);
}

#[tokio::test]
async fn stand_completion_retries_failed_server_save_without_losing_timer() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = stand_fixture().await;
        let seed = (0..=255).find(|seed| stompymux_rs::BattleDice::seeded([*seed;32]).two_d6() >= 6).unwrap();
        shot_seed(&mut world, id, seed);
        let _attempt = stompymux_rs::begin_battle_stand(&mut world, id, ObjectId(1), stompymux_rs::BattleStandMode::Normal, true, fall_rules()).unwrap();
        for _ in 0..4 { stompymux_rs::advance_battle_standing(&mut world); }
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_stand BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'stand failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_stand").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].stand_timer().is_none()).await;
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn stand_timer_corruption_and_destroyed_gyro_are_rejected_without_rolls() {
    let (_dir, config, world, id) = stand_fixture().await;
    for (state_name, remaining) in [("recovering", 0), ("recovering", 61), ("rising", 5)] {
        let mut corrupt = world.clone();
        let mut state = serde_json::to_value(&corrupt.btech).unwrap();
        state["constructed"][id.0.to_string()]["stand_timer"] =
            serde_json::json!({"state":state_name,"remaining":remaining});
        corrupt.btech = serde_json::from_value(state).unwrap();
        assert!(corrupt.validate(&config).is_err());
    }
    let mut corrupt = world.clone();
    let mut state = serde_json::to_value(&corrupt.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["posture"] = serde_json::json!("standing");
    unit["stand_timer"] = serde_json::json!({"state":"rising","remaining":5});
    unit["motion"]["desired_speed"] = serde_json::json!(1.0);
    corrupt.btech = serde_json::from_value(state).unwrap();
    assert!(corrupt.validate(&config).is_err());
    let mut world = world;
    let gyros: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == stompymux_rs::BattleSystem::Gyro)
        .map(|part| part.location)
        .take(2)
        .collect();
    for location in gyros {
        stompymux_rs::destroy_battle_critical(&mut world, id, location).unwrap();
    }
    let before = world.btech.clone();
    assert!(
        stompymux_rs::begin_battle_stand(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleStandMode::Anyway,
            true,
            fall_rules()
        )
        .unwrap_err()
        .to_string()
        .contains("destroyed gyro")
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn first_pilot_injury_replays_from_prepared_dice_after_failed_save() {
    use sqlx::Connection;
    let (_dir, config, mut world, id) = fixture('.').await;
    assert_eq!(
        world.btech.recoveries()[&ObjectId(1)].mode,
        stompymux_rs::BattleRecoveryMode::Ready
    );
    let before = world.btech.clone();
    stompymux_rs::prepare_battle_recovery(&mut world, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let checkpoint = world.clone();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER deny_first_injury BEFORE UPDATE ON btech_character_recovery BEGIN SELECT RAISE(ABORT,'injury failure'); END").execute(&mut sql).await.unwrap();
    let injury = stompymux_rs::injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap();
    let expected = world.btech.clone();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        before
    );
    world = checkpoint;
    sqlx::query("DROP TRIGGER deny_first_injury")
        .execute(&mut sql)
        .await
        .unwrap();
    assert_eq!(
        stompymux_rs::injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap(),
        injury
    );
    assert_eq!(world.btech, expected);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        expected
    );
}

#[tokio::test]
async fn startup_commits_missing_character_and_pilot_dice_before_gameplay() {
    use sqlx::Connection;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, _id) = fixture('.').await;
            stompymux_rs::set_battle_character(
                &mut world,
                ObjectId(2),
                stompymux_rs::BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 3,
                    learn: 2,
                    charisma: 1,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut sql = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
            )
            .await
            .unwrap();
            sqlx::query("DELETE FROM btech_character_recovery")
                .execute(&mut sql)
                .await
                .unwrap();
            assert!(
                persistence::load(&config.database())
                    .await
                    .unwrap()
                    .btech
                    .recoveries()
                    .is_empty()
            );
            let (_address, shutdown, task, _lua, _heartbeats) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            let loaded = persistence::load(&config.database()).await.unwrap();
            for player in [ObjectId(1), ObjectId(2)] {
                assert_eq!(
                    loaded.btech.recoveries()[&player].mode,
                    stompymux_rs::BattleRecoveryMode::Ready
                );
                assert_eq!(loaded.btech.recoveries()[&player].remaining, 0);
            }
            let ready = loaded.btech.recoveries().clone();
            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            task.await.unwrap().unwrap();
            let (_address, shutdown, task, _lua, _heartbeats) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            assert_eq!(
                persistence::load(&config.database())
                    .await
                    .unwrap()
                    .btech
                    .recoveries(),
                &ready
            );
            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

#[tokio::test]
async fn damage_balance_uses_each_actuator_penalty_and_skips_arms_and_broken_hips() {
    use stompymux_rs::{
        BattleSection as Section, CriticalLocation, resolve_battle_tactical_impact,
    };
    let (_dir, config, mut base, id) = fixture('.').await;
    balance_skill(&mut base);
    for slot in 1..=4 {
        let mut world = base.clone();
        let seed = single_critical_seed(4, slot);
        shot_seed(&mut world, id, seed);
        let mut dice = stompymux_rs::BattleDice::seeded([seed; 32]);
        dice.two_d6(); // Material entry.
        dice.two_d6();
        dice.die(4).unwrap();
        let expected = dice.two_d6();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(Section::LeftLeg, true),
            1,
            fall_rules(),
        )
        .unwrap();
        assert_eq!(
            report.impact.criticals[0].0,
            CriticalLocation {
                section: Section::LeftLeg,
                slot: (slot - 1) as u8
            }
        );
        assert_eq!(report.balance.len(), 1);
        let check = report.balance[0].check.unwrap();
        assert_eq!(check.damage, if slot == 1 { 2 } else { 1 });
        assert_eq!(check.roll, Some(expected));
        assert!(check.success);
        assert!(report.balance[0].fall.is_none());
        world.validate(&config).unwrap();
    }
    for (section, hip_lost, count, selection) in [
        (Section::LeftArm, false, 4, 2),
        (Section::LeftLeg, true, 3, 1),
    ] {
        let mut world = base.clone();
        if hip_lost {
            stompymux_rs::destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation { section, slot: 0 },
            )
            .unwrap();
        }
        let seed = single_critical_seed(count, selection);
        shot_seed(&mut world, id, seed);
        let mut dice = stompymux_rs::BattleDice::seeded([seed; 32]);
        dice.two_d6(); // Material entry.
        dice.two_d6();
        dice.die(count).unwrap();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(section, true),
            1,
            fall_rules(),
        )
        .unwrap();
        assert!(report.balance.is_empty());
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(dice).unwrap()
        );
    }
}

#[tokio::test]
async fn damage_balance_checks_first_gyro_before_second_gyro_forces_a_fall() {
    use stompymux_rs::{
        BattleBalanceCause, BattlePosture, BattleSection as Section, BattleSystem,
        resolve_battle_tactical_impact,
    };
    let (_dir, config, mut base, id) = fixture('.').await;
    balance_skill(&mut base);
    let mut chosen = None;
    for seed in 0..=255 {
        let mut world = base.clone();
        shot_seed(&mut world, id, seed);
        let before = world.clone();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(Section::CenterTorso, true),
            1,
            fall_rules(),
        )
        .unwrap();
        if report.balance.len() == 2
            && report.balance.iter().all(|entry| {
                matches!(
                    entry.cause,
                    BattleBalanceCause::Critical {
                        system: BattleSystem::Gyro,
                        ..
                    }
                )
            })
        {
            chosen = Some((before, world, report));
            break;
        }
    }
    let (before, world, report) = chosen.expect("two gyro hits in one TAC cascade");
    let first = &report.balance[0];
    assert_eq!(first.check.unwrap().damage, 3);
    assert_eq!(first.check.unwrap().skill, -19);
    assert!(first.check.unwrap().success);
    assert!(first.fall.is_none());
    let second = &report.balance[1];
    assert!(second.check.is_none());
    assert_eq!(second.fall.as_ref().unwrap().damage, 4);
    assert!(
        second
            .fall
            .as_ref()
            .unwrap()
            .avoidance
            .unwrap()
            .roll
            .is_some()
    );
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    assert_eq!(
        resolve_battle_tactical_impact(
            &mut loaded,
            id,
            balance_hit(Section::CenterTorso, true),
            1,
            fall_rules()
        )
        .unwrap(),
        report
    );
    assert_eq!(loaded.btech, world.btech);
}

#[tokio::test]
async fn damage_balance_failed_gyro_check_falls_once_and_prone_hits_do_not_repeat_it() {
    use stompymux_rs::{BattlePosture, BattleSection as Section, resolve_battle_tactical_impact};
    let (_dir, config, mut world, id) = fixture('.').await;
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            dice.two_d6(); // Material entry.
            matches!(dice.two_d6(), 8 | 9)
                && (4..=7).contains(&dice.die(12).unwrap())
                && dice.two_d6() < 9
        })
        .unwrap();
    shot_seed(&mut world, id, seed);
    let report = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(Section::CenterTorso, true),
        1,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(report.balance.len(), 1);
    let check = report.balance[0].check.unwrap();
    assert!(!check.success);
    assert_eq!(check.target, 9);
    assert!(report.balance[0].fall.is_some());
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    assert_eq!(
        report
            .notices
            .iter()
            .filter(|notice| notice.text == "You lose your balance and fall down!")
            .count(),
        1
    );
    shot_seed(&mut world, id, single_critical_seed(4, 1));
    let again = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(Section::LeftLeg, true),
        1,
        fall_rules(),
    )
    .unwrap();
    assert!(again.balance.is_empty());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn damage_balance_leg_loss_forces_falls_on_ground_and_bridge_decks() {
    use stompymux_rs::{
        BattleBalanceCause, BattlePosture, BattleSection as Section, resolve_battle_tactical_impact,
    };
    for terrain in ['.', '/'] {
        let (_dir, config, mut world, id) = fixture(terrain).await;
        balance_skill(&mut world);
        let seed = (0..=255)
            .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 8)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let result = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(Section::LeftLeg, false),
            14,
            fall_rules(),
        );
        let report = result.unwrap();
        assert_eq!(report.balance.len(), 1);
        assert_eq!(
            report.balance[0].cause,
            BattleBalanceCause::SectionLost(Section::LeftLeg)
        );
        assert!(report.balance[0].check.is_none());
        assert!(
            report.balance[0]
                .fall
                .as_ref()
                .unwrap()
                .avoidance
                .unwrap()
                .success
        );
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
        assert_eq!(
            world.btech.constructed_units()[&id].sections()[&Section::LeftLeg].internal,
            0
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn damage_balance_direct_salvo_uses_fallen_facing_and_replays_after_failed_save() {
    use sqlx::Connection;
    use stompymux_rs::{
        BattleBalanceCause, BattleDamagePhase, BattleDice, BattleHitArc, BattleSection as Section,
        apply_damage_phase, resolve_battle_shot, resolve_battle_tactical_impact, roll_unit_dice,
    };
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    stompymux_rs::set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Missile",
        stompymux_rs::BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    apply_damage_phase(
        &mut base,
        target,
        Section::RightLeg,
        6,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    apply_damage_phase(
        &mut base,
        target,
        Section::RightLeg,
        7,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    let mut chosen = None;
    for seed in 0..=255 {
        let mut world = base.clone();
        shot_seed(&mut world, target, seed);
        let before = world.clone();
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, 4, shot_rules()).unwrap();
        let Some(stompymux_rs::BattleTargetSalvo::Mech(salvo)) = &report.salvo else {
            continue;
        };
        if salvo.groups.len() < 2
            || salvo.groups[0].balance.first().is_none_or(|entry| {
                entry.cause != BattleBalanceCause::SectionLost(Section::RightLeg)
            })
        {
            continue;
        }
        let first = &salvo.groups[0];
        // Reproduce the first group, then derive the next hit from the new facing and remaining dice.
        let mut intermediate = before.clone();
        roll_unit_dice(&mut intermediate, target, 2).unwrap(); // Cluster roll.
        roll_unit_dice(&mut intermediate, target, 2).unwrap(); // First ordinary location roll.
        let first_impact = resolve_battle_tactical_impact(
            &mut intermediate,
            target,
            first.hit,
            first.damage,
            fall_rules(),
        )
        .unwrap();
        assert_eq!(first_impact.balance, first.balance);
        let heading = intermediate.btech.constructed_units()[&target]
            .motion()
            .unwrap()
            .heading;
        let bearing = stompymux_rs::battle_unit_range(&intermediate, target, id)
            .unwrap()
            .bearing
            .unwrap();
        let arc = BattleHitArc::from_bearing(bearing, heading, 0).unwrap();
        if arc == BattleHitArc::Rear {
            continue;
        }
        let roll = roll_unit_dice(&mut intermediate, target, 2)
            .unwrap()
            .into_iter()
            .sum::<u8>();
        let unit = &intermediate.btech.constructed_units()[&target];
        let mut dice: BattleDice =
            serde_json::from_value(serde_json::to_value(unit).unwrap()["dice"].clone()).unwrap();
        let mut stale_dice = dice.clone();
        let expected = shot_rules()
            .hit
            .resolve(unit, arc, roll, &mut dice)
            .unwrap();
        let stale = shot_rules()
            .hit
            .resolve(unit, BattleHitArc::Rear, roll, &mut stale_dice)
            .unwrap();
        if expected == stale {
            continue;
        }
        assert_eq!(salvo.groups[1].hit, expected);
        chosen = Some((before, world, report));
        break;
    }
    let (before, world, report) =
        chosen.expect("a first-missile leg loss that changes the next impact arc");
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_balance BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'balance failure'); END;").execute(&mut sql).await.unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, before.btech);
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    assert_eq!(
        resolve_battle_shot(&mut loaded, id, ObjectId(1), target, 4, shot_rules()).unwrap(),
        report
    );
    assert_eq!(loaded.btech, world.btech);
    sqlx::query("DROP TRIGGER reject_balance")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn damage_balance_ammunition_fall_precedes_explosion_pilot_injury() {
    use stompymux_rs::{BattleDice, BattleSection as Section, resolve_battle_tactical_impact};
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let ammunition = template
        .sections
        .get_mut(&Section::RightTorso)
        .unwrap()
        .criticals
        .remove(&0)
        .unwrap();
    template
        .sections
        .get_mut(&Section::RightLeg)
        .unwrap()
        .criticals
        .insert(4, ammunition);
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    // This scenario needs depleted live ammunition, independently of template initialization.
    world.btech.set_unit_ammunition_bin(id, 0, 1).unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let _injury = stompymux_rs::injure_battle_tactical_pilot(&mut world, id, 3, false).unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6(); // Initial material entry.
            if !matches!(dice.two_d6(), 8 | 9) || dice.die(5).unwrap() != 5 {
                return false;
            }
            dice.two_d6(); // The ammunition explosion enters its own damage path.
            if dice.two_d6() >= 8 {
                return false;
            }
            dice.d6(); // Forced fall on a stopped unit: no unit piloting dice.
            dice.two_d6() != 12 // Keep the fall itself away from the head.
        })
        .unwrap();
    shot_seed(&mut world, id, seed);
    let report = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(Section::RightLeg, true),
        1,
        fall_rules(),
    )
    .unwrap();
    let fall = report.balance[0].fall.as_ref().unwrap();
    assert_eq!(fall.pilot_injury.as_ref().unwrap().injuries, 4);
    assert_eq!(fall.groups.len(), 1);
    assert_eq!(report.pilot_injuries.len(), 1);
    assert!(report.pilot_injuries[0].killed);
    assert_eq!(world.btech.constructed_units()[&id].pilot_injuries(), 6);
    assert!(report.impact.destroyed);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn prone_pivots_and_stand_countdown_turns_preserve_position_and_restart() {
    use stompymux_rs::{BattleDice, BattleStandMode, BattleStandTimer, begin_battle_stand};
    let (_dir, config, base, id) = stand_fixture().await;
    for mode in [None, Some(true), Some(false)] {
        for fasa_turning in [false, true] {
            let mut world = base.clone();
            if let Some(success) = mode {
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = BattleDice::seeded([*seed; 32]);
                        (dice.two_d6() >= 6) == success && dice.two_d6() >= 7
                    })
                    .unwrap();
                shot_seed(&mut world, id, seed);
                let attempt = begin_battle_stand(
                    &mut world,
                    id,
                    ObjectId(1),
                    BattleStandMode::Normal,
                    true,
                    fall_rules(),
                )
                .unwrap();
                assert_eq!(attempt.check.success, success);
                assert!(
                    attempt
                        .fall
                        .as_ref()
                        .is_none_or(|fall| fall.pilot_injury.is_none())
                );
                assert!(matches!(
                    (success, attempt.timer),
                    (true, Some(BattleStandTimer::Rising { .. }))
                        | (false, Some(BattleStandTimer::Recovering { .. }))
                ));
            }
            let unit = &world.btech.constructed_units()[&id];
            let old = unit.motion().unwrap();
            let old_position = unit.position();
            let timer = unit.stand_timer();
            let dice = serde_json::to_value(unit).unwrap()["dice"].clone();
            let turn = 1.5 * unit.mobility().maximum_speed / 10.75;
            let desired = (old.heading + 90.0).rem_euclid(360.0);
            set_battle_heading(&mut world, id, ObjectId(1), desired).unwrap();
            world.validate(&config).unwrap();
            let rules = BattleMovementRules {
                fasa_turning,
                slowdown: 2,
                ..stompymux_rs::BattleMovementRules::STANDARD
            };
            assert!(advance_battle_motion(&mut world, rules).unwrap().is_empty());
            let unit = &world.btech.constructed_units()[&id];
            assert!(
                (unit.motion().unwrap().heading - (old.heading + turn).rem_euclid(360.0)).abs()
                    < 1e-9
            );
            assert_eq!(unit.motion().unwrap().point, old.point);
            assert_eq!(unit.position(), old_position);
            assert_eq!(unit.motion().unwrap().speed, 0.0);
            assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
            assert_eq!(unit.stand_timer(), timer);
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            for _ in 0..60 {
                advance_battle_motion(&mut world, rules).unwrap();
                advance_battle_motion(&mut loaded, rules).unwrap();
            }
            assert_eq!(world.btech, loaded.btech);
            assert_eq!(
                world.btech.constructed_units()[&id]
                    .motion()
                    .unwrap()
                    .heading,
                desired
            );
            assert_eq!(
                world.btech.constructed_units()[&id].motion().unwrap().point,
                old.point
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn prone_pivot_guards_preserve_state_and_stand_timers_still_reject_travel() {
    let (_dir, config, mut world, id) = stand_fixture().await;
    let before = world.btech.clone();
    assert!(set_battle_heading(&mut world, id, ObjectId(2), 90.0).is_err());
    assert!(set_battle_heading(&mut world, id, ObjectId(1), f64::NAN).is_err());
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
    assert_eq!(world.btech, before);
    for (timer, posture) in [("rising", "standing"), ("recovering", "prone")] {
        for field in ["speed", "desired_speed"] {
            let mut invalid = world.clone();
            let mut state = serde_json::to_value(&invalid.btech).unwrap();
            let unit = &mut state["constructed"][id.0.to_string()];
            unit["posture"] = posture.into();
            unit["stand_timer"] = serde_json::json!({"state": timer, "remaining": 5});
            unit["motion"][field] = 1.0.into();
            invalid.btech = serde_json::from_value(state).unwrap();
            assert!(invalid.validate(&config).is_err());
        }
    }
    let gyros: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == stompymux_rs::BattleSystem::Gyro)
        .take(2)
        .map(|part| part.location)
        .collect();
    for gyro in gyros {
        stompymux_rs::destroy_battle_critical(&mut world, id, gyro).unwrap();
    }
    let before = world.btech.clone();
    assert!(set_battle_heading(&mut world, id, ObjectId(1), 90.0).is_err());
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

/// Keep posture tests away from unrelated head injury and through-armor-critical rolls.
fn prone_fire_fall_seed() -> u8 {
    (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            dice.two_d6();
            dice.d6();
            (3..=11).contains(&dice.two_d6())
        })
        .unwrap()
}

/// A prone shooter aimed at its acquired target, with safe fall dice and usable pilot skills.
async fn prone_shot_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, id, target) = shot_fixture().await;
    balance_skill(&mut world);
    shot_skill(&mut world, 30);
    let seed = prone_fire_fall_seed();
    shot_seed(&mut world, id, seed);
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 0.0).unwrap();
    for _ in 0..30 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    stompymux_rs::refresh_battle_contacts(&mut world, &[id]).unwrap();
    (dir, config, world, id, target)
}

#[tokio::test]
async fn prone_fire_reserves_opposite_arm_through_recycle_and_restart() {
    use stompymux_rs::{advance_battle_recycle, resolve_battle_shot};
    let (_dir, config, mut world, id, target) = prone_shot_fixture().await;
    // Two torso hits keep this support test independent of random target criticals.
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            dice.two_d6() == 7 && dice.two_d6() == 7
        })
        .unwrap();
    shot_seed(&mut world, target, seed);
    let shot = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).unwrap();
    assert_eq!(shot.aim.attacker_movement, 2);
    let unit = &world.btech.constructed_units()[&id];
    assert!(!unit.weapon_readiness(2).unwrap().posture_ready);
    assert!(unit.weapon_readiness(1).unwrap().ready);
    assert!(unit.weapon_readiness(4).unwrap().posture_ready);
    let before = world.btech.clone();
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 2, shot_rules()).is_err());
    assert_eq!(world.btech, before);
    let _second =
        resolve_battle_shot(&mut world, id, ObjectId(1), target, 1, shot_rules()).unwrap();
    for _ in 0..9 {
        advance_battle_recycle(&mut world);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert!(
        !loaded.btech.constructed_units()[&id]
            .weapon_readiness(2)
            .unwrap()
            .ready
    );
    for _ in 0..11 {
        advance_battle_recycle(&mut world);
        advance_battle_recycle(&mut loaded);
    }
    assert_eq!(loaded.btech, world.btech);
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(2)
            .unwrap()
            .ready
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn prone_fire_support_depends_on_surviving_arms_and_all_their_weapon_timers() {
    use stompymux_rs::{
        BattleDamagePhase as P, BattleSection as S, CriticalLocation, apply_damage_phase,
        destroy_battle_critical, spend_battle_weapon,
    };
    let (_dir, config, mut base, id, _target) = prone_shot_fixture().await;
    destroy_battle_critical(
        &mut base,
        id,
        CriticalLocation {
            section: S::LeftArm,
            slot: 0,
        },
    )
    .unwrap();
    assert!(
        base.btech.constructed_units()[&id]
            .weapon_readiness(4)
            .unwrap()
            .posture_ready
    );
    for busy in [false, true] {
        let mut world = base.clone();
        if busy {
            spend_battle_weapon(&mut world, id, ObjectId(1), 1).unwrap();
        }
        apply_damage_phase(&mut world, id, S::RightArm, 100, P::Internal).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert!(!unit.weapon_readiness(0).unwrap().posture_ready);
        assert_eq!(unit.weapon_readiness(4).unwrap().posture_ready, !busy);
        if busy {
            let before = world.btech.clone();
            assert!(spend_battle_weapon(&mut world, id, ObjectId(1), 4).is_err());
            assert_eq!(world.btech, before);
        }
        apply_damage_phase(&mut world, id, S::LeftArm, 100, P::Internal).unwrap();
        assert!(
            !world.btech.constructed_units()[&id]
                .weapon_readiness(4)
                .unwrap()
                .posture_ready
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn prone_fire_forbids_leg_mounts_until_upright() {
    use stompymux_rs::{BattleSection as S, BattleStandMode, begin_battle_stand};
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let weapon = template
        .sections
        .get_mut(&S::LeftArm)
        .unwrap()
        .criticals
        .remove(&2)
        .unwrap();
    template
        .sections
        .get_mut(&S::RightLeg)
        .unwrap()
        .criticals
        .insert(4, weapon);
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    balance_skill(&mut world);
    shot_seed(&mut world, id, prone_fire_fall_seed());
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    let index = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|weapon| weapon.criticals[0].section == S::RightLeg)
        .unwrap();
    assert!(
        !world.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .posture_ready
    );
    let before = world.btech.clone();
    assert!(stompymux_rs::spend_battle_weapon(&mut world, id, ObjectId(1), index).is_err());
    assert_eq!(world.btech, before);
    let _stand = begin_battle_stand(
        &mut world,
        id,
        ObjectId(1),
        BattleStandMode::Normal,
        true,
        fall_rules(),
    )
    .unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn prone_fire_cannot_cross_the_waterline_before_expenditure() {
    let (_dir, config, mut world, id, target) = prone_shot_fixture().await;
    let map = world.create(&config, "Shallow water".into(), Kind::Room);
    let source = format!("12 12\n{}", format!("{}\n", "~1".repeat(12)).repeat(12));
    create_battle_map(
        &mut world,
        map,
        "shallow.map",
        MapAsset::from_cells(&source).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 5, 5).unwrap();
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    assert!(
        stompymux_rs::battle_unit_terrain_los(&world, id, target)
            .unwrap()
            .blocked
    );
    let before = world.btech.clone();
    assert!(
        stompymux_rs::resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules())
            .unwrap_err()
            .to_string()
            .contains("Target is not a current acquired contact")
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn stagger_rolling_modes_consume_whole_groups_and_retain_prior_difficulty() {
    use stompymux_rs::{BattleSection as S, BattleStaggerMode as M, advance_battle_stagger};
    let (_dir, config, base, id) = stagger_fixture().await;
    for mode in [M::Retain, M::Consume] {
        let mut world = base.clone();
        let rules = stagger_rules(mode);
        stagger_hit(&mut world, id, S::LeftTorso, 13, mode);
        stagger_hit(&mut world, id, S::RightTorso, 14, mode);
        for _ in 0..3 {
            assert!(
                advance_battle_stagger(&mut world, rules)
                    .unwrap()
                    .is_empty()
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        world = persistence::load(&config.database()).await.unwrap();
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(stompymux_rs::Flag::Connected);
        assert!(
            advance_battle_stagger(&mut world, rules)
                .unwrap()
                .is_empty()
        );
        let report = advance_battle_stagger(&mut world, rules).unwrap();
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].level, 1);
        assert_eq!(report[0].check.situational, -2); // Assault tonnage at level one.
        assert!(report[0].check.success);
        let hits = &world.btech.constructed_units()[&id].stagger().hits;
        if mode == M::Consume {
            assert!(hits.is_empty());
        } else {
            assert_eq!(hits.len(), 2);
            assert!(hits.iter().all(|hit| hit.counted)); // Entire 27 points, not a split 20/7.
        }
        stagger_hit(&mut world, id, S::CenterTorso, 20, mode);
        for _ in 0..4 {
            assert!(
                advance_battle_stagger(&mut world, rules)
                    .unwrap()
                    .is_empty()
            );
        }
        let report = advance_battle_stagger(&mut world, rules).unwrap();
        assert_eq!(report[0].level, if mode == M::Retain { 2 } else { 1 });
        let dice =
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
        for _ in 0..60 {
            assert!(
                advance_battle_stagger(&mut world, rules)
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(
            world.btech.constructed_units()[&id]
                .stagger()
                .hits
                .is_empty()
        );
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            dice
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn stagger_traditional_checks_once_per_turn_and_persists_its_phase() {
    use stompymux_rs::{BattleSection as S, BattleStaggerMode as M, advance_battle_stagger};
    let (_dir, config, mut world, id) = stagger_fixture().await;
    let rules = stagger_rules(M::Traditional);
    stagger_hit(&mut world, id, S::LeftTorso, 20, M::Traditional);
    let first = advance_battle_stagger(&mut world, rules).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].check.situational, 1);
    stagger_hit(&mut world, id, S::RightTorso, 20, M::Traditional);
    for _ in 0..29 {
        assert!(
            advance_battle_stagger(&mut world, rules)
                .unwrap()
                .is_empty()
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(stompymux_rs::Flag::Connected);
    let next = advance_battle_stagger(&mut world, rules).unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(advance_battle_stagger(&mut loaded, rules).unwrap(), next);
    assert_eq!(loaded.btech, world.btech);
    for _ in 0..30 {
        assert!(
            advance_battle_stagger(&mut world, rules)
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        world.btech.constructed_units()[&id]
            .stagger()
            .checked_phase
            .is_none()
    );
    stagger_hit(&mut world, id, S::CenterTorso, 19, M::Traditional);
    for _ in 0..30 {
        assert!(
            advance_battle_stagger(&mut world, rules)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        world.btech.constructed_units()[&id].stagger().turn_damage,
        0
    );
}

#[tokio::test]
async fn stagger_failed_checks_fall_and_clear_history_on_ground_and_bridge_decks() {
    use stompymux_rs::{
        BattlePosture, BattleSection as S, BattleStaggerMode as M, advance_battle_stagger,
    };
    for terrain in ['.', '/'] {
        let source = format!(
            "12 12\n{}",
            format!("{}\n", format!("{terrain}0").repeat(12)).repeat(12)
        );
        let (_dir, config, mut world, id) = fixture_assets(
            &source,
            BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml"))
                .unwrap(),
        )
        .await;
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        stagger_hit(&mut world, id, S::LeftTorso, 20, M::Retain);
        let rules = stagger_rules(M::Retain);
        for _ in 0..4 {
            assert!(
                advance_battle_stagger(&mut world, rules)
                    .unwrap()
                    .is_empty()
            );
        }
        let result = advance_battle_stagger(&mut world, rules);
        {
            let reports = result.unwrap();
            assert_eq!(reports[0].check.roll, None);
            assert_eq!(reports[0].check.situational, 999);
            assert!(reports[0].fall.is_some());
            assert_eq!(
                world.btech.constructed_units()[&id].posture(),
                BattlePosture::Prone
            );
            assert!(
                world.btech.constructed_units()[&id]
                    .stagger()
                    .hits
                    .is_empty()
            );
            assert!(
                reports[0]
                    .notices
                    .iter()
                    .any(|notice| notice.text.contains("fall over"))
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn stagger_counts_original_hit_once_and_excludes_prone_damage() {
    use stompymux_rs::{BattleSection as S, BattleStaggerMode as M};
    let (_dir, config, mut world, id) = stagger_fixture().await;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 8)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let report = stompymux_rs::resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(S::LeftArm, false),
        60,
        fall_rules(),
    )
    .unwrap();
    assert!(
        report
            .impact
            .phases
            .iter()
            .any(|phase| phase.section == S::LeftTorso)
    );
    assert_eq!(world.btech.constructed_units()[&id].stagger().hits.len(), 1);
    assert_eq!(
        world.btech.constructed_units()[&id].stagger().hits[0].damage,
        60
    );
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            dice.two_d6();
            dice.d6();
            (3..=11).contains(&dice.two_d6()) && (3..=11).contains(&dice.two_d6())
        })
        .unwrap();
    shot_seed(&mut world, id, seed);
    let _fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .stagger()
            .hits
            .is_empty()
    );
    stagger_hit(&mut world, id, S::RightArm, 1, M::Retain);
    assert!(
        world.btech.constructed_units()[&id]
            .stagger()
            .hits
            .is_empty()
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn stagger_invalid_windows_and_overflow_hits_are_atomic() {
    use stompymux_rs::{BattleSection as S, BattleStaggerHit};
    let (_dir, config, world, id) = stagger_fixture().await;
    for (damage, remaining) in [(0, 60), (1, 0), (1, 61)] {
        let mut invalid = world.clone();
        invalid
            .btech
            .rewrite_unit_record(id, |record| {
                record["stagger"]["hits"] = serde_json::json!([BattleStaggerHit {
                    damage,
                    remaining,
                    counted: false
                }]);
            })
            .unwrap();
        assert!(invalid.validate(&config).is_err());
    }
    let mut world = world;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["stagger"]["hits"] = serde_json::to_value(vec![
                BattleStaggerHit {
                    damage: 1,
                    remaining: 60,
                    counted: false
                };
                4096
            ])
            .unwrap();
        })
        .unwrap();
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    assert!(
        stompymux_rs::resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(S::LeftTorso, false),
            1,
            fall_rules()
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn stagger_server_water_fall_retries_failed_save_without_losing_history() {
    use sqlx::Connection;
    use std::{cell::Cell, rc::Rc};
    use stompymux_rs::{
        BattlePosture, BattleSection as S, BattleStaggerMode as M, advance_battle_stagger,
    };
    tokio::task::LocalSet::new().run_until(async {
        let source = format!("12 12\n{}", format!("{}\n", "~1".repeat(12)).repeat(12));
        let (_dir, config, mut world, id) = fixture_assets(&source, BattleTemplate::parse("AS7-D",include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap()).await;
        shot_seed(&mut world, id, water_fall_seed(false));
        stop_battle_unit(&mut world, id, ObjectId(1), stompymux_rs::BattleMovementRules::STANDARD.fall).unwrap();
        stagger_hit(&mut world, id, S::LeftTorso, 20, M::Retain);
        for _ in 0..4 { assert!(advance_battle_stagger(&mut world, stagger_rules(M::Retain)).unwrap().is_empty()); }
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        stompymux_rs::advance_battle_reactor_windows(&mut world);
        let mut expected_rules = stagger_rules(M::Retain);
        expected_rules.hit = stompymux_rs::BattleFallRules::configured(&config).hit;
        let _expected = advance_battle_stagger(&mut world, expected_rules).unwrap();
        // The successful retry commits exactly one global turn phase; rejected ticks remain at zero.
        let mut phase_state = serde_json::to_value(&world.btech).unwrap();
        phase_state["turn_clock"] = 1.into();
        phase_state["simulation_seconds"] = 1.into();
        world.btech = serde_json::from_value(phase_state).unwrap();
        let expected = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_stagger BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'stagger failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _scripts, mut heartbeats) = support::start(&config, Rc::new(Cell::new(0))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER reject_stagger").execute(&mut sql).await.unwrap();
        let current = heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].posture() == BattlePosture::Prone).await.btech;
        assert_eq!(current, expected);
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn stagger_exact_expiry_threshold_and_optional_tonnage_are_observable() {
    use stompymux_rs::{BattleSection as S, BattleStaggerMode as M, advance_battle_stagger};
    for heavy in [false, true] {
        let (_dir, config, mut base, id) = if heavy {
            stagger_fixture().await
        } else {
            fixture('.').await
        };
        balance_skill(&mut base);
        let mut rules = stagger_rules(M::Retain);
        rules.interval = 1;
        let mut world = base.clone();
        for (section, damage) in [(S::LeftTorso, 8), (S::RightTorso, 8), (S::CenterTorso, 3)] {
            stagger_hit(&mut world, id, section, damage, M::Retain);
        }
        let dice =
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
        for _ in 0..59 {
            assert!(
                advance_battle_stagger(&mut world, rules)
                    .unwrap()
                    .is_empty()
            );
        }
        assert!(
            world.btech.constructed_units()[&id]
                .stagger()
                .hits
                .iter()
                .all(|hit| hit.remaining == 1)
        );
        assert!(
            advance_battle_stagger(&mut world, rules)
                .unwrap()
                .is_empty()
        );
        assert!(
            world.btech.constructed_units()[&id]
                .stagger()
                .hits
                .is_empty()
        );
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            dice
        );
        for tonnage in [true, false] {
            let mut world = base.clone();
            for (section, damage) in [(S::LeftTorso, 8), (S::RightTorso, 8), (S::CenterTorso, 4)] {
                stagger_hit(&mut world, id, section, damage, M::Retain);
            }
            rules.tonnage = tonnage;
            let reports = advance_battle_stagger(&mut world, rules).unwrap();
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0].level, 1);
            assert_eq!(
                reports[0].check.situational,
                if !tonnage {
                    0
                } else if heavy {
                    -2
                } else {
                    1
                }
            );
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn water_falls_scale_damage_and_replay_before_standing() {
    use stompymux_rs::{BattlePosture, BattleStandMode, begin_battle_stand, resolve_battle_fall};
    for depth in [0, 1, 2] {
        let (_dir, config, mut world, id) = water_fixture(depth).await;
        persistence::save(&config.database(), &world).await.unwrap();
        let fall = resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
        assert_eq!(fall.damage, if depth == 0 { 4 } else { 2 });
        assert!(fall.feedback.flooding.is_empty());
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        loaded
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(stompymux_rs::Flag::Connected);
        assert_eq!(
            resolve_battle_fall(&mut loaded, id, 1, fall_rules()).unwrap(),
            fall
        );
        assert_eq!(loaded.btech, world.btech);
        let stand = begin_battle_stand(
            &mut world,
            id,
            ObjectId(1),
            BattleStandMode::Normal,
            true,
            fall_rules(),
        )
        .unwrap();
        assert!(stand.check.success);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn water_leg_breaches_force_falls_without_destroying_structure() {
    use stompymux_rs::{
        BattleSection as S, BattleStandMode, CriticalLocation, begin_battle_stand,
        resolve_battle_tactical_impact,
    };
    let (_dir, config, mut world, id) = water_fixture(1).await;
    let result = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(S::LeftLeg, false),
        6,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(result.flooding.len(), 1);
    assert_eq!(result.flooding[0].section, S::LeftLeg);
    assert_eq!(result.flooding[0].fall.as_ref().unwrap().damage, 2);
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.sections()[&S::LeftLeg].internal, 8);
    assert!(!unit.critical_destroyed(CriticalLocation {
        section: S::LeftLeg,
        slot: 0
    }));
    assert!(unit.critical_unavailable(CriticalLocation {
        section: S::LeftLeg,
        slot: 0
    }));
    assert_eq!(unit.mobility().maximum_speed, 10.75);
    assert_eq!(unit.mobility().piloting_modifier, 5);
    let stand = begin_battle_stand(
        &mut world,
        id,
        ObjectId(1),
        BattleStandMode::Normal,
        true,
        fall_rules(),
    )
    .unwrap();
    assert!(stand.check.success);
    for _ in 0..30 {
        stompymux_rs::advance_battle_standing(&mut world);
    }
    assert!(
        stompymux_rs::flood_battle_unit(&mut world, id, fall_rules())
            .unwrap()
            .is_empty()
    );
    shot_seed(&mut world, id, single_critical_seed(4, 3));
    let actuator = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(S::LeftLeg, true),
        1,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(
        actuator.impact.criticals[0].0,
        CriticalLocation {
            section: S::LeftLeg,
            slot: 2
        }
    );
    assert!(actuator.balance.is_empty()); // The flooded hip is already unavailable.
    shot_seed(&mut world, id, water_fall_seed(true));
    let _second = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(S::RightLeg, false),
        6,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .mobility()
            .maximum_speed,
        0.0
    );
    assert!(
        begin_battle_stand(
            &mut world,
            id,
            ObjectId(1),
            BattleStandMode::Normal,
            true,
            fall_rules()
        )
        .unwrap_err()
        .to_string()
        .contains("No legs")
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn water_depth_and_rear_breaches_disable_ammo_and_engine_without_explosions() {
    use stompymux_rs::{
        BattleDamagePhase as P, BattleSection as S, BattleSystem, apply_damage_phase,
        flood_battle_unit,
    };
    for depth in [1, 2] {
        let (_dir, config, mut world, id) = water_fixture(depth).await;
        apply_damage_phase(&mut world, id, S::RightTorso, 4, P::Armor { rear: true }).unwrap();
        let reports = flood_battle_unit(&mut world, id, fall_rules()).unwrap();
        if depth == 1 {
            assert!(reports.is_empty());
            assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[25]);
            let fall = stompymux_rs::resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
            assert!(
                fall.feedback
                    .flooding
                    .iter()
                    .any(|report| report.section == S::RightTorso)
            );
        } else {
            assert_eq!(reports[0].section, S::RightTorso);
        }
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.ammunition(), &[0]);
        assert_eq!(unit.sections()[&S::RightTorso].internal, 8);
        assert!(unit.lost_criticals().is_empty());
        assert_eq!(unit.system_hits(BattleSystem::JumpJet), 2);
        assert!(!unit.weapon_readiness(4).unwrap().ready);
        assert!(
            flood_battle_unit(&mut world, id, fall_rules())
                .unwrap()
                .is_empty()
        );
        world.validate(&config).unwrap();
    }
    let (_dir, config, mut world, id) = water_fixture(2).await;
    apply_damage_phase(&mut world, id, S::CenterTorso, 10, P::Armor { rear: false }).unwrap();
    let reports = flood_battle_unit(&mut world, id, fall_rules()).unwrap();
    assert_eq!(reports[0].section, S::CenterTorso);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.is_destroyed());
    assert_eq!(unit.power(), BattlePower::Off);
    assert_eq!(unit.sections()[&S::CenterTorso].internal, 11);
    assert!(unit.lost_criticals().is_empty());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn water_flooded_equipment_stays_disabled_after_restart_and_leaving_water() {
    use stompymux_rs::{
        BattleDamagePhase as P, BattleSection as S, CriticalLocation, apply_damage_phase,
        resolve_battle_tactical_impact,
    };
    let (_dir, config, mut world, id) = water_fixture(2).await;
    let report = resolve_battle_tactical_impact(
        &mut world,
        id,
        balance_hit(S::LeftArm, false),
        4,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(report.flooding[0].section, S::LeftArm);
    assert_eq!(
        report
            .notices
            .iter()
            .filter(|notice| notice.text.contains("Water floods"))
            .count(),
        1
    );
    assert!(
        !world.btech.constructed_units()[&id]
            .weapon_intact(0)
            .unwrap()
    );
    let location = CriticalLocation {
        section: S::LeftArm,
        slot: 2,
    };
    assert!(!world.btech.constructed_units()[&id].critical_destroyed(location));
    assert!(
        world.btech.constructed_units()[&id]
            .critical_candidates(S::LeftArm)
            .contains(&location)
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    stop_battle_unit(
        &mut loaded,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let map = loaded.create(&config, "Dry repair field".into(), Kind::Room);
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    create_battle_map(
        &mut loaded,
        map,
        "dry.map",
        MapAsset::from_cells(&source).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut loaded, map, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut loaded, id, map, 5, 5).unwrap();
    assert!(
        loaded.btech.constructed_units()[&id]
            .flooded_sections()
            .contains(&S::LeftArm)
    );
    assert!(
        !loaded.btech.constructed_units()[&id]
            .weapon_intact(0)
            .unwrap()
    );
    apply_damage_phase(&mut loaded, id, S::LeftArm, 6, P::Internal).unwrap();
    assert!(
        !loaded.btech.constructed_units()[&id]
            .flooded_sections()
            .contains(&S::LeftArm)
    );
    assert!(loaded.btech.constructed_units()[&id].critical_destroyed(location));
    loaded.validate(&config).unwrap();
}

#[tokio::test]
async fn water_initial_flooding_precedes_fall_damage_and_disables_external_sinks() {
    use stompymux_rs::{
        BattleDamagePhase as P, BattleSection as S, apply_damage_phase, flood_battle_unit,
        resolve_battle_fall,
    };
    let (_dir, config, mut world, id) = water_fixture(2).await;
    let baseline = world.clone();
    let before = world.btech.constructed_units()[&id]
        .heat_rates(&world)
        .dissipation;
    apply_damage_phase(&mut world, id, S::Head, 7, P::Armor { rear: false }).unwrap();
    let reports = flood_battle_unit(&mut world, id, fall_rules()).unwrap();
    assert_eq!(reports[0].section, S::Head);
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.heat_rates(&world).dissipation, before - 1.0);
    assert_eq!(unit.pilot_injuries(), 0); // Flooding destroys the unit without adding injury points.
    assert!(unit.is_destroyed());
    assert_eq!(unit.pilot(), None);
    world = baseline;
    apply_damage_phase(&mut world, id, S::CenterTorso, 10, P::Armor { rear: false }).unwrap();
    shot_seed(&mut world, id, water_fall_seed(false));
    let fall = resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    assert_eq!(fall.feedback.flooding[0].section, S::CenterTorso);
    assert!(fall.groups.is_empty()); // Engine flooding already destroyed the unit.
    assert!(fall.feedback.flooding[0].reactor_explosion.is_none());
    assert_eq!(fall.direction_roll, 4); // The eligible flooding check consumes dice before fall direction.
    assert_eq!(
        world.btech.constructed_units()[&id].sections()[&S::CenterTorso].internal,
        11
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn direct_glancing_modes_resolve_exact_boundaries_and_replay_after_restart() {
    use stompymux_rs::{BattleGlancingMode as G, resolve_battle_shot};
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut base, id, seed);
    // An ordinary torso location keeps the comparison independent of pilot protection rolls.
    shot_seed(&mut base, target, seed);
    for (mode, ordinary_target, damage, glancing) in [
        (G::Disabled, 7, Some(5), false),
        (G::Disabled, 8, None, false),
        (G::AtTarget, 6, Some(5), false),
        (G::AtTarget, 7, Some(3), true),
        (G::AtTarget, 8, None, false),
        (G::BelowTarget, 7, Some(5), false),
        (G::BelowTarget, 8, Some(3), true),
        (G::BelowTarget, 9, None, false),
    ] {
        let mut world = base.clone();
        shot_skill(&mut world, 9 - ordinary_target);
        let mut rules = shot_rules();
        rules.glancing = mode;
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let report = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
        assert_eq!(report.roll, 7);
        assert_eq!(report.target_number, Some(i32::from(ordinary_target)));
        assert_eq!(report.glancing, glancing);
        assert_eq!(
            report
                .notices()
                .iter()
                .filter(|notice| notice.text == "You destroyed the target!")
                .count(),
            usize::from(world.btech.constructed_units()[&target].is_destroyed())
        );
        assert_eq!(
            report
                .notices()
                .iter()
                .filter(|notice| notice.text == "You have been destroyed!")
                .count(),
            usize::from(world.btech.constructed_units()[&target].is_destroyed())
        );
        assert_eq!(
            report
                .notices()
                .iter()
                .filter(|notice| notice.text == "You are nicked by a glancing blow!")
                .count(),
            usize::from(glancing)
        );
        assert_eq!(
            report
                .salvo
                .as_ref()
                .and_then(|salvo| salvo.as_mech())
                .map(|salvo| salvo.groups.iter().map(|group| group.damage).sum::<u16>()),
            damage
        );
        assert_eq!(world.btech.constructed_units()[&id].heat().stored, 3.0);
        assert_eq!(
            world.btech.constructed_units()[&id].weapon_recycle()[&0],
            20
        );
        if damage.is_none() {
            assert_eq!(
                world.btech.constructed_units()[&target],
                before.constructed_units()[&target]
            );
        }
        let mut replay = persistence::load(&config.database()).await.unwrap();
        replay
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(stompymux_rs::Flag::Connected);
        let repeated = resolve_battle_shot(&mut replay, id, ObjectId(1), target, 0, rules).unwrap();
        assert_eq!(report, repeated);
        assert_eq!(world.btech, replay.btech);
        world.validate(&config).unwrap();
    }
    for (setting, expected) in [
        (0, G::Disabled),
        (1, G::AtTarget),
        (2, G::BelowTarget),
        (-1, G::AtTarget),
        (3, G::AtTarget),
    ] {
        assert_eq!(G::from_setting(setting), expected);
    }
}

#[tokio::test]
async fn direct_glancing_missiles_spend_full_salvo_and_apply_reduced_clusters() {
    use stompymux_rs::{BattleGlancingMode, BattleWeapon, resolve_battle_shot};
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 2);
    stompymux_rs::set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Missile",
        stompymux_rs::BattleCharacterValue {
            value: 2,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut base, id, seed);
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Srm4)
        .unwrap();
    for (cluster, count) in [(5, 1), (6, 1), (7, 2), (12, 3)] {
        let target_seed = (0..=255)
            .find(|seed| {
                let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
                dice.two_d6() == cluster && (0..count).all(|_| (3..12).contains(&dice.two_d6()))
            })
            .unwrap();
        let mut world = base.clone();
        shot_seed(&mut world, target, target_seed);
        let before = world.btech.constructed_units()[&id].clone();
        let mut rules = shot_rules();
        rules.glancing = BattleGlancingMode::AtTarget;
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, rules).unwrap();
        assert_eq!(report.target_number, Some(7));
        assert!(report.glancing);
        let salvo = report.salvo.unwrap().into_mech().unwrap();
        assert_eq!(salvo.cluster_roll, Some(cluster));
        assert_eq!(salvo.groups.len(), count);
        assert!(salvo.groups.iter().all(|group| group.damage == 2));
        let after = &world.btech.constructed_units()[&id];
        let bin = report.expenditure.ammunition[0].bin_index;
        assert_eq!(after.ammunition()[bin], before.ammunition()[bin] - 1);
        assert_eq!(after.heat().stored, 3.0);
        assert_eq!(after.weapon_recycle()[&index], 15);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn fire_command_lists_weapons_rejects_invalid_requests_and_uses_configured_glancing() {
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 2);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut world, id, seed);
    shot_seed(&mut world, target, seed);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let listing = support::run_text(&scripts, &config, ObjectId(1), 1, "weapons");
    assert!(listing.contains("0: IS.MediumLaser"), "{listing}");
    assert!(listing.contains("ready; 0s"), "{listing}");
    for line in [
        "fire",
        "fire -1",
        "fire one",
        "fire 0",
        "fire 0 #999999",
        "fire 999999 #1",
        "fire 0 target",
        "fire 0 #1 extra",
        "fire/nope 0",
        "weapons/nope",
        "weapons extra",
    ] {
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, line);
        assert!(!output.contains("You fire"), "{line}: {output}");
        assert_eq!(scripts.world().btech, before, "{line}");
    }
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{target}", target = target.0),
    );
    assert!(text.contains("BTH: 7 Roll: 7. Glancing hit."), "{text}");
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .target_lock()
            .is_none()
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].weapon_recycle()[&0],
        20
    );
    let after = scripts.world().btech.clone();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    assert!(text.contains("still recharging"), "{text}");
    assert_eq!(scripts.world().btech, after);
}

#[tokio::test]
async fn fire_command_uses_selected_target_and_requires_its_conscious_pilot() {
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 20);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut world, target, seed);
    stompymux_rs::select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "fire 0");
    assert!(text.contains("cockpit"), "{text}");
    assert_eq!(scripts.world().btech, before);
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "fire 0");
    assert!(text.contains(&format!("at #{}", target.0)), "{text}");
    assert!(text.contains("Hit."), "{text}");
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .target_lock()
            .unwrap()
            .target,
        target
    );
}

async fn fire_command_matrix(cases: &[(bool, bool)]) {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        for (use_lua, lethal) in cases.iter().copied() {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        shot_skill(&mut world, 20);
        stompymux_rs::set_battle_character_value(&mut world, ObjectId(1), "Gunnery-Missile", stompymux_rs::BattleCharacterValue { value: 20, experience: 0, last_used: 0 }).unwrap();
        let index = world.btech.constructed_units()[&id].loadout().unwrap().weapons.iter().position(|mount| mount.weapon == stompymux_rs::BattleWeapon::Srm4).unwrap();
        let seed = (0..=255).find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7).unwrap();
        shot_seed(&mut world, id, seed);
        shot_seed(&mut world, target, seed);
        if lethal {
            // Damage the CT before selecting a lethal missile stream under the configured rules.
            stompymux_rs::apply_damage_phase(&mut world, target, stompymux_rs::BattleSection::CenterTorso, u16::MAX, stompymux_rs::BattleDamagePhase::Armor { rear: false }).unwrap();
            let structure = world.btech.constructed_units()[&target].sections()[&stompymux_rs::BattleSection::CenterTorso].internal;
            stompymux_rs::apply_damage_phase(&mut world, target, stompymux_rs::BattleSection::CenterTorso, structure - 1, stompymux_rs::BattleDamagePhase::Internal).unwrap();
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
            assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            let seed = (0..=255).find(|seed| {
                let mut probe = world.clone();
                shot_seed(&mut probe, target, *seed);
                let _report = stompymux_rs::resolve_battle_shot(&mut probe, id, ObjectId(1), target, index, configured_shot_rules(&config)).unwrap();
                probe.btech.constructed_units()[&target].is_destroyed()
            }).unwrap();
            shot_seed(&mut world, target, seed);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("UPDATE player_state SET password_hash=? WHERE object_dbref=1").bind(stompymux_rs::accounts::hash("secret", &config).unwrap()).execute(&mut sql).await.unwrap();
        if use_lua {
            let source = "return {commands={{name='test-btech-fire',permission='everyone',pattern='^test%-btech%-fire$',handler=function(ctx) btech.unit.fire(UNIT,ctx.enactor,WEAPON,TARGET) end}}}".replace("UNIT", &id.0.to_string()).replace("WEAPON", &index.to_string()).replace("TARGET", &target.0.to_string());
            std::fs::write(config.path("lua/global_logic/test_btech_fire.lua"), source).unwrap();
        }
        let command = if use_lua { "test-btech-fire".to_owned() } else { format!("fire {index} #{}", target.0) };
        let (address, shutdown, task, _lua, _heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = support::Client { socket: tokio::net::TcpStream::connect(address).await.unwrap(), pending: Vec::new() };
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Jenner").await;
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_fire BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'fire save failure'); END", target.0))).execute(&mut sql).await.unwrap();
        let before = persistence::load(&config.database()).await.unwrap();
        client.send(&command).await;
        let failure = client.until("Unable to save your changes.").await;
        client.send("say __failed_fire_complete__").await;
        let failure = format!("{failure}{}", client.until("__failed_fire_complete__").await);
        assert!(!failure.contains("You fire"), "{failure}");
        assert!(!failure.contains("You destroyed the target!"), "{failure}");
        let rejected = persistence::load(&config.database()).await.unwrap();
        assert!(!rejected.btech.constructed_units()[&target].is_destroyed());
        assert_eq!(rejected.btech.constructed_units()[&target].pilot(), before.btech.constructed_units()[&target].pilot());
        assert_eq!(rejected.btech.constructed_units()[&target].sections(), before.btech.constructed_units()[&target].sections());
        assert_eq!(rejected.btech.constructed_units()[&id].weapon_recycle(), before.btech.constructed_units()[&id].weapon_recycle());
        let dice = |world: &stompymux_rs::World, unit: ObjectId| serde_json::to_value(&world.btech).unwrap()["constructed"][unit.0.to_string()]["dice"].clone();
        assert_eq!(rejected.btech.constructed_units()[&id].ammunition(), before.btech.constructed_units()[&id].ammunition());
        assert_eq!(dice(&rejected, id), dice(&before, id));
        assert_eq!(dice(&rejected, target), dice(&before, target));
        sqlx::query("DROP TRIGGER reject_fire").execute(&mut sql).await.unwrap();
        client.send(&command).await;
        let success = client.until(if lethal { "You destroyed the target!" } else { "Hit." }).await;
        client.send("say __retried_fire_complete__").await;
        let success = format!("{success}{}", client.until("__retried_fire_complete__").await);
        assert_eq!(success.matches("You fire").count(), 1, "{success}");
        assert_eq!(success.matches("You destroyed the target!").count(), usize::from(lethal), "{success}");
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech.constructed_units()[&target].is_destroyed(), lethal);
        if lethal { assert_eq!(saved.btech.constructed_units()[&target].pilot(), None); }
        assert_ne!(saved.btech.constructed_units()[&target].sections(), before.btech.constructed_units()[&target].sections());
        assert!(saved.btech.constructed_units()[&id].weapon_recycle()[&index] > 0);
        assert_eq!(saved.btech.constructed_units()[&id].ammunition()[0], before.btech.constructed_units()[&id].ammunition()[0] - 1);
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        }
    }).await;
}

#[tokio::test]
async fn fire_command_failed_server_save_discards_native_survivable_fire() {
    fire_command_matrix(&[(false, false)]).await;
}

#[tokio::test]
async fn fire_command_failed_server_save_discards_lua_survivable_fire() {
    fire_command_matrix(&[(true, false)]).await;
}

#[tokio::test]
async fn fire_command_failed_server_save_discards_native_lethal_fire() {
    fire_command_matrix(&[(false, true)]).await;
}

#[tokio::test]
async fn fire_command_failed_server_save_discards_lua_lethal_fire() {
    fire_command_matrix(&[(true, true)]).await;
}

#[tokio::test]
async fn ammunition_hazard_selection_prefers_damage_then_first_bin_and_skips_unavailable() {
    use stompymux_rs::{BattleCriticalLoss, BattleSection as S};
    let (_dir, _config, mut world, id) = fixture_assets(
        &format!("12 12\n{}", (".0".repeat(12) + "\n").repeat(12)),
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .await;
    let before = world.btech.clone();
    let first = world.btech.constructed_units()[&id]
        .ammunition_hazard_maximum()
        .unwrap()
        .unwrap();
    assert_eq!(first.damage, 180);
    assert_eq!(world.btech, before);
    let bins = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .ammunition;
    // This Atlas carries one 180-damage SRM bin, two 120-damage LRM bins and two 100-damage AC bins.
    assert_eq!(first.index, 0);
    let loss = stompymux_rs::destroy_battle_critical(&mut world, id, first.location)
        .unwrap()
        .unwrap();
    assert!(matches!(
        loss,
        BattleCriticalLoss::Ammunition {
            explosion_damage: 180,
            ..
        }
    ));
    let next = world.btech.constructed_units()[&id]
        .ammunition_hazard_maximum()
        .unwrap()
        .unwrap();
    assert_eq!(next.index, 1);
    assert_eq!(next.damage, 120);
    assert_ne!(next.location.section, S::Head);
    for bin in bins {
        stompymux_rs::destroy_battle_critical(&mut world, id, bin.location).unwrap();
    }
    assert!(
        world.btech.constructed_units()[&id]
            .ammunition_hazard_maximum()
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn ammunition_detonation_bypasses_armor_replays_and_honors_pain_resistance() {
    use stompymux_rs::{BattleSection as S, explode_battle_ammunition};
    let (_dir, config, mut base, id) = fixture('.').await;
    shot_skill(&mut base, 20);
    // One SRM4 salvo has eight internal damage, destroying the torso without reaching CT.
    base.btech.set_unit_ammunition_bin(id, 0, 1).unwrap();
    for (resistance, injuries) in [(0, 2), (1, 1)] {
        let mut world = base.clone();
        stompymux_rs::set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Pain_Resistance",
            stompymux_rs::BattleCharacterValue {
                value: resistance,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let report = explode_battle_ammunition(&mut world, id, 0, fall_rules()).unwrap();
        assert_eq!(report.pilot_injuries.last().unwrap().injuries, injuries);
        assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[0]);
        assert_eq!(
            world.btech.constructed_units()[&id].sections()[&S::RightTorso].internal,
            0
        );
        assert_eq!(
            world.btech.constructed_units()[&id].sections()[&S::CenterTorso],
            base.btech.constructed_units()[&id].sections()[&S::CenterTorso]
        );
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text == "Ammunition explosion!")
        );
        assert!(
            world.btech.constructed_units()[&id]
                .stagger()
                .hits
                .is_empty()
        );
        let mut replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            explode_battle_ammunition(&mut replay, id, 0, fall_rules()).unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        let before = world.btech.clone();
        assert!(explode_battle_ammunition(&mut world, id, 0, fall_rules()).is_err());
        assert!(explode_battle_ammunition(&mut world, id, usize::MAX, fall_rules()).is_err());
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn ammunition_detonation_leg_falls_on_ground_and_bridge_decks() {
    use stompymux_rs::{BattleSection as S, explode_battle_ammunition};
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let bin = template
        .sections
        .get_mut(&S::RightTorso)
        .unwrap()
        .criticals
        .remove(&0)
        .unwrap();
    assert!(bin.equipment.starts_with("Ammo_"));
    template
        .sections
        .get_mut(&S::RightLeg)
        .unwrap()
        .criticals
        .insert(4, bin);
    for terrain in ['.', '/'] {
        let source = format!(
            "12 12\n{}",
            (format!("{terrain}0").repeat(12) + "\n").repeat(12)
        );
        let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
        balance_skill(&mut world);
        world.btech.set_unit_ammunition_bin(id, 0, 1).unwrap();
        shot_seed(&mut world, id, water_fall_seed(false));
        let result = explode_battle_ammunition(&mut world, id, 0, fall_rules());
        let report = result.unwrap();
        assert_eq!(report.balance.len(), 1);
        assert!(report.balance[0].fall.is_some());
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            stompymux_rs::BattlePosture::Prone
        );
        assert!(
            world.btech.constructed_units()[&id]
                .stagger()
                .hits
                .is_empty()
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn flooded_ammunition_cannot_be_selected_or_detonated_as_a_heat_hazard() {
    use stompymux_rs::{BattleSection, explode_battle_ammunition};
    let (_dir, config, mut world, id) = water_fixture(2).await;
    let bin = world.btech.constructed_units()[&id]
        .ammunition_hazard_maximum()
        .unwrap()
        .unwrap();
    let hit = stompymux_rs::BattleHit {
        section: BattleSection::RightTorso,
        rear_armor: true,
        through_armor_critical: false,
        crew_stun: false,
    };
    let _report =
        stompymux_rs::resolve_battle_tactical_impact(&mut world, id, hit, 4, fall_rules()).unwrap();
    assert!(!world.btech.constructed_units()[&id].critical_destroyed(bin.location));
    assert!(
        world.btech.constructed_units()[&id]
            .ammunition_hazard_maximum()
            .unwrap()
            .is_none()
    );
    let before = world.btech.clone();
    assert!(explode_battle_ammunition(&mut world, id, bin.index, fall_rules()).is_err());
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn overheat_clock_restarts_mid_turn_and_each_sample_is_consumed_once() {
    use stompymux_rs::{advance_battle_heat, advance_battle_overheat};
    let (_dir, config, mut world, id) = fixture('.').await;
    assert!(!world.btech.constructed_units()[&id].heat_active(&world));
    assert!(world.btech.constructed_units()[&id].overheat_active());
    computer_skill(&mut world, 30);
    // Constant fourteen-point excess, supplying the heat lost between samples.
    for step in 1..=15 {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["heat"]["stored"] = 24.0.into();
            })
            .unwrap();
        advance_battle_heat(&mut world);
        assert!(
            advance_battle_overheat(&mut world, overheat_rules())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            world.btech.constructed_units()[&id]
                .overheat_clock()
                .elapsed,
            step
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for step in 16..=30 {
        let mut outputs = Vec::new();
        for candidate in [&mut world, &mut replay] {
            candidate
                .btech
                .rewrite_unit_record(id, |record| {
                    record["heat"]["stored"] = 24.0.into();
                })
                .unwrap();
            advance_battle_heat(candidate);
            outputs.push(advance_battle_overheat(candidate, overheat_rules()).unwrap());
        }
        assert_eq!(world.btech, replay.btech);
        assert_eq!(outputs[0], outputs[1]);
        if step < 30 {
            assert!(outputs[0].is_empty());
        } else {
            let check = outputs[0][0].shutdown_check.unwrap();
            assert!(check.success && check.computer);
            assert_eq!(check.target, -17);
            assert!(
                outputs[0][0]
                    .messages()
                    .iter()
                    .any(|(_, text)| text.contains("Modified skill BTH: -17 Roll:"))
            );
            assert_eq!(
                world.btech.constructed_units()[&id]
                    .overheat_clock()
                    .elapsed,
                0
            );
        }
    }
    let before = world.btech.clone();
    assert!(
        advance_battle_overheat(&mut world, overheat_rules())
            .unwrap()
            .is_empty()
    );
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn overheat_unpiloted_thresholds_and_computer_dice_match_heat_bands() {
    use stompymux_rs::advance_battle_overheat;
    let (_dir, config, mut base, id) = fixture('.').await;
    // Empty bins retain their slots; a failed ammunition check still consumes its roll.
    base.btech.set_unit_ammunition_bin(id, 0, 0).unwrap();
    for (heat, ammo_target, reactor_target) in [
        (10.0, None, 13),
        (13.99, None, 13),
        (14.0, None, 4),
        (18.0, None, 6),
        (19.0, Some(4), 6),
        (22.0, Some(4), 8),
        (23.0, Some(6), 8),
        (26.0, Some(6), 10),
        (28.0, Some(8), 10),
        (30.0, Some(8), 13),
    ] {
        let mut world = base.clone();
        stompymux_rs::release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        overheat_due(&mut world, id, heat, false);
        shot_seed(&mut world, id, 32);
        let mut dice = stompymux_rs::BattleDice::seeded([32; 32]);
        let ammo_roll = ammo_target.map(|_| dice.two_d6());
        let shutdown_roll = (reactor_target != 13).then(|| dice.two_d6());
        let reports = advance_battle_overheat(&mut world, overheat_rules()).unwrap();
        let report = &reports[0];
        assert_eq!(
            report.ammunition_check.map(|check| check.target),
            ammo_target
        );
        assert_eq!(
            report.ammunition_check.and_then(|check| check.roll),
            ammo_roll
        );
        let check = report.shutdown_check.unwrap();
        assert_eq!(check.target, reactor_target);
        assert_eq!(check.roll, shutdown_roll);
        assert_eq!(report.shutdown, !check.success);
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()["constructed"][id.0.to_string()]["dice"],
            serde_json::to_value(dice).unwrap()
        );
        world.validate(&config).unwrap();
    }
    for (skill, target) in [(0, 21), (4, 17), (30, -9)] {
        let mut world = base.clone();
        computer_skill(&mut world, skill);
        overheat_due(&mut world, id, 30.0, false);
        shot_seed(&mut world, id, 32);
        let mut dice = stompymux_rs::BattleDice::seeded([32; 32]);
        dice.two_d6(); // Ammunition avoidance comes first.
        let roll = if skill == 0 {
            let rolls = [dice.d6(), dice.d6(), dice.d6()];
            rolls.iter().sum::<u8>() - rolls.iter().max().unwrap()
        } else {
            dice.two_d6()
        };
        let report = advance_battle_overheat(&mut world, overheat_rules())
            .unwrap()
            .remove(0);
        let check = report.shutdown_check.unwrap();
        assert!(check.computer);
        assert_eq!(check.target, target);
        assert_eq!(check.roll, Some(roll));
        assert_eq!(report.shutdown, i16::from(roll) < target);
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()["constructed"][id.0.to_string()]["dice"],
            serde_json::to_value(dice).unwrap()
        );
    }
}

#[tokio::test]
async fn overheat_life_support_injury_precedes_ammunition_explosion_and_shutdown() {
    use stompymux_rs::{BattleSection as S, advance_battle_overheat};
    let (_dir, config, mut world, id) = fixture('.').await;
    computer_skill(&mut world, 30);
    stompymux_rs::destroy_battle_critical(
        &mut world,
        id,
        stompymux_rs::CriticalLocation {
            section: S::Head,
            slot: 0,
        },
    )
    .unwrap();
    world.btech.set_unit_ammunition_bin(id, 0, 1).unwrap();
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() < 6)
        .unwrap();
    shot_seed(&mut world, id, seed);
    overheat_due(&mut world, id, 26.0, true);
    let report = advance_battle_overheat(&mut world, overheat_rules())
        .unwrap()
        .remove(0);
    assert_eq!(report.injury.unwrap().injuries, 2);
    assert_eq!(
        report
            .explosion
            .as_ref()
            .unwrap()
            .pilot_injuries
            .last()
            .unwrap()
            .injuries,
        4
    );
    assert!(!report.ammunition_check.unwrap().success);
    assert!(report.shutdown_check.unwrap().success);
    assert!(!report.shutdown);
    assert_eq!(
        report.notices[0].text,
        "You take personal injury from heat!"
    );
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[0]);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn overheat_shutdown_at_speed_can_topple_without_structural_fall_damage() {
    use stompymux_rs::advance_battle_overheat;
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = stompymux_rs::BattleDice::seeded([*seed; 32]);
            for _ in 0..3 {
                dice.d6();
            }
            dice.two_d6() < 9 && dice.two_d6() >= 6
        })
        .unwrap();
    for (terrain, speed) in [
        ('.', 10.75_f64),
        ('.', -10.75),
        ('.', 10.8),
        ('.', -10.8),
        ('/', 10.8),
    ] {
        let (_dir, config, mut world, id) = fixture(terrain).await;
        world
            .btech
            .edit_unit_motion(id, |motion| {
                motion.speed = speed;
                motion.desired_speed = speed;
            })
            .unwrap();
        overheat_due(&mut world, id, 14.0, false);
        shot_seed(&mut world, id, seed);
        let before = world.btech.clone();
        let result = advance_battle_overheat(&mut world, overheat_rules());
        let report = result.unwrap().remove(0);
        assert!(report.shutdown);
        assert_eq!(report.fall.is_some(), speed.abs() > 10.75);
        if let Some(fall) = report.fall {
            assert_eq!(fall.damage, 0);
            assert!(fall.groups.is_empty());
            assert!(fall.pilot_injury.is_none());
        }
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.sections(), before.constructed_units()[&id].sections());
        assert_eq!(unit.power(), BattlePower::Off);
        assert!(unit.pilot().is_none());
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn overheat_intact_life_support_uses_one_coin_roll_above_thirty_heat() {
    use stompymux_rs::advance_battle_overheat;
    let (_dir, config, mut base, id) = fixture('.').await;
    computer_skill(&mut base, 30);
    base.btech.set_unit_ammunition_bin(id, 0, 0).unwrap();
    for coin in [1, 2] {
        let seed = (0..=255)
            .find(|seed| {
                stompymux_rs::BattleDice::seeded([*seed; 32])
                    .die(2)
                    .unwrap()
                    == coin
            })
            .unwrap();
        let mut world = base.clone();
        overheat_due(&mut world, id, 30.01, true);
        shot_seed(&mut world, id, seed);
        let mut dice = stompymux_rs::BattleDice::seeded([seed; 32]);
        dice.die(2).unwrap();
        let ammunition_roll = dice.two_d6();
        let shutdown_roll = dice.two_d6();
        let report = advance_battle_overheat(&mut world, overheat_rules())
            .unwrap()
            .remove(0);
        assert_eq!(
            report.injury.map(|injury| injury.injuries),
            (coin == 1).then_some(1)
        );
        assert_eq!(report.ammunition_check.unwrap().roll, Some(ammunition_roll));
        assert_eq!(report.shutdown_check.unwrap().roll, Some(shutdown_roll));
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()["constructed"][id.0.to_string()]["dice"],
            serde_json::to_value(dice).unwrap()
        );
        world.validate(&config).unwrap();
    }
    for clock in [
        serde_json::json!({"elapsed":31,"phase":0,"injury_due":false}),
        serde_json::json!({"elapsed":0,"phase":30,"injury_due":false}),
        serde_json::json!({"elapsed":0,"phase":1,"injury_due":true}),
    ] {
        let mut invalid = base.clone();
        invalid
            .btech
            .rewrite_unit_record(id, |record| {
                record["overheat_clock"] = clock;
            })
            .unwrap();
        assert!(
            invalid
                .validate(&config)
                .unwrap_err()
                .to_string()
                .contains("overheat clock")
        );
    }
}

#[tokio::test]
async fn overheat_server_retries_shutdown_without_advancing_the_failed_clock_or_dice() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture('.').await;
        overheat_due(&mut world, id, 14.0, false);
        world.btech
            .rewrite_unit_record(id, |record| {
        record["overheat_clock"] = serde_json::json!({"elapsed":29,"phase":29,"injury_due":false});
        })
            .unwrap();
        shot_seed(&mut world, id, 32);
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        stompymux_rs::advance_battle_reactor_windows(&mut world);
        stompymux_rs::advance_battle_heat(&mut world);
        let reports = stompymux_rs::advance_battle_overheat(&mut world, overheat_rules()).unwrap();
        assert!(reports[0].shutdown);
        // The successful retry commits exactly one global turn phase; rejected ticks remain at zero.
        let mut phase_state = serde_json::to_value(&world.btech).unwrap();
        phase_state["turn_clock"] = 1.into();
        phase_state["simulation_seconds"] = 1.into();
        world.btech = serde_json::from_value(phase_state).unwrap();
        let expected = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_overheat BEFORE UPDATE ON btech_units WHEN COALESCE(json_extract(NEW.live,'$.power.state'), 'off') = 'off' BEGIN SELECT RAISE(ABORT,'overheat save failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER reject_overheat").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].power() == BattlePower::Off).await;
        assert_eq!(saved.btech, expected);
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn lua_firing_matches_native_commands_and_returns_detached_weapon_and_shot_tables() {
    for selected in [false, true] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        shot_skill(&mut world, 2);
        let seed = (0..=255)
            .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
            .unwrap();
        shot_seed(&mut world, id, seed);
        shot_seed(&mut world, target, seed);
        if selected {
            stompymux_rs::select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
        }
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let inspection: (usize, String, bool) = scripts.eval_callback(&format!("local w=btech.unit.weapon_states({id}); assert(w[1].index==0); w[1].index=100; w[1].readiness.ready=false; local actual=btech.unit.weapon_states({id})[1]; return actual.index,actual.name,actual.readiness.ready", id=id.0)).unwrap();
        assert_eq!(inspection, (0, "IS.MediumLaser".into(), true));
        assert_eq!(scripts.world().btech, before);
        let command = if selected {
            "fire 0".to_owned()
        } else {
            format!("fire 0 #{}", target.0)
        };
        let native_text = support::run_text(&native, &config, ObjectId(1), 1, &command);
        let argument = if selected {
            "nil".to_owned()
        } else {
            target.0.to_string()
        };
        let report: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.fire({},1,0,{argument})", id.0))
            .unwrap();
        assert_eq!(report.get::<u8>("roll").unwrap(), 7);
        assert_eq!(report.get::<bool>("glancing").unwrap(), !selected);
        assert_eq!(report.get::<i64>("target").unwrap(), target.0);
        assert!(matches!(
            report.get::<mlua::Value>("dice").unwrap(),
            mlua::Value::Nil
        ));
        assert_eq!(scripts.world().btech, native.world().btech);
        let messages = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, text)| text.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(messages, native_text);
        report.set("roll", 0).unwrap();
        report
            .get::<mlua::Table>("salvo")
            .unwrap()
            .get::<mlua::Table>("report")
            .unwrap()
            .get::<mlua::Table>("groups")
            .unwrap()
            .get::<mlua::Table>(1)
            .unwrap()
            .set("damage", 9999)
            .unwrap();
        assert_eq!(scripts.world().btech, native.world().btech);
        let ready: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.weapon_states({})[1].readiness.ready",
                id.0
            ))
            .unwrap();
        assert!(!ready);
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn lua_firing_errors_and_callback_abort_restore_damage_dice_and_output() {
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 20);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut world, target, seed);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.fire({},1,0,{}); error('abort firing callback')",
                id.0, target.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    for (pilot, weapon, selected) in [
        (2, 0, target.0),
        (1, 999, target.0),
        (1, 0, id.0),
        (1, 0, 999999),
    ] {
        let code: String = scripts.eval_callback(&format!("local ok,e=pcall(btech.unit.fire,{},{pilot},{weapon},{selected}); assert(not ok); return e.code", id.0)).unwrap();
        assert_eq!(code, "btech.operation.failed");
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.fire({},1,-1,{})", id.0, target.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let outside: String = scripts
        .inspect_lua()
        .load(format!(
            "local ok,e=pcall(btech.unit.fire,{},1,0,{}); assert(not ok); return e.code",
            id.0, target.0
        ))
        .eval()
        .unwrap();
    assert_eq!(outside, "mux.state.unavailable");
    let checking = scripts
        .from_sources_for_inspection(
            &config,
            stompymux_rs::help::HelpIndex::load(&config).unwrap(),
            std::sync::Arc::new(stompymux_rs::LuaSources::read(&config).unwrap()),
            stompymux_rs::RuntimeMode::Checking,
        )
        .unwrap();
    for call in [
        format!("btech.unit.fire,{},1,0,{}", id.0, target.0),
        format!("btech.unit.weapon_states,{}", id.0),
    ] {
        let code: String = checking
            .inspect_lua()
            .load(format!(
                "local ok,e=pcall({call}); assert(not ok); return e.code"
            ))
            .eval()
            .unwrap();
        assert_eq!(code, "mux.unavailable.checking");
    }
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn lua_firing_misses_and_optional_state_use_nil_without_losing_expenditure() {
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 0);
    let seed = (0..=255)
        .find(|seed| stompymux_rs::BattleDice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let before_target = world.btech.constructed_units()[&target].clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let result: (bool, bool, bool, u8) = scripts.eval_callback(&format!("local shot=btech.unit.fire({},1,0,{}); local state=btech.unit.state({}); return shot.salvo==nil,#shot.expenditure.ammunition==0,state.target_lock==nil and state.stand_timer==nil,shot.roll", id.0,target.0,id.0)).unwrap();
    assert_eq!(result, (true, true, true, 7));
    assert_eq!(
        scripts.world().btech.constructed_units()[&target],
        before_target
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].weapon_recycle()[&0],
        20
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].heat().stored,
        3.0
    );
    let absent: bool = scripts
        .eval_callback(&format!(
            "btech.unit.release({},1); return btech.unit.state({}).pilot==nil",
            id.0, id.0
        ))
        .unwrap();
    assert!(absent);
}

/// Conventional direct-fire matrix coverage; sharded twelve weapons per test.
const CONVENTIONAL_DIRECT_FIRE_WEAPONS: [stompymux_rs::BattleWeapon; 107] = [
    stompymux_rs::BattleWeapon::ClanLbx2,
    stompymux_rs::BattleWeapon::ClanLbx5,
    stompymux_rs::BattleWeapon::ClanLbx10,
    stompymux_rs::BattleWeapon::ClanLbx20,
    stompymux_rs::BattleWeapon::ClanUltraAc2,
    stompymux_rs::BattleWeapon::ClanUltraAc5,
    stompymux_rs::BattleWeapon::ClanUltraAc10,
    stompymux_rs::BattleWeapon::ClanUltraAc20,
    stompymux_rs::BattleWeapon::Thunderbolt5,
    stompymux_rs::BattleWeapon::Thunderbolt10,
    stompymux_rs::BattleWeapon::Thunderbolt15,
    stompymux_rs::BattleWeapon::Thunderbolt20,
    stompymux_rs::BattleWeapon::HyperAc2,
    stompymux_rs::BattleWeapon::HyperAc5,
    stompymux_rs::BattleWeapon::HyperAc10,
    stompymux_rs::BattleWeapon::ClanErLargeLaser,
    stompymux_rs::BattleWeapon::ClanErMediumLaser,
    stompymux_rs::BattleWeapon::ClanErSmallLaser,
    stompymux_rs::BattleWeapon::ClanErMicroLaser,
    stompymux_rs::BattleWeapon::ClanErPpc,
    stompymux_rs::BattleWeapon::ClanFlamer,
    stompymux_rs::BattleWeapon::ClanHeavyLargeLaser,
    stompymux_rs::BattleWeapon::ClanHeavyMediumLaser,
    stompymux_rs::BattleWeapon::ClanHeavySmallLaser,
    stompymux_rs::BattleWeapon::ClanLargePulseLaser,
    stompymux_rs::BattleWeapon::ClanMediumPulseLaser,
    stompymux_rs::BattleWeapon::ClanSmallPulseLaser,
    stompymux_rs::BattleWeapon::ClanMicroPulseLaser,
    stompymux_rs::BattleWeapon::ClanErLargePulseLaser,
    stompymux_rs::BattleWeapon::ClanErMediumPulseLaser,
    stompymux_rs::BattleWeapon::ClanErSmallPulseLaser,
    stompymux_rs::BattleWeapon::ClanPlasmaRifle,
    stompymux_rs::BattleWeapon::ClanGaussRifle,
    stompymux_rs::BattleWeapon::ClanMachineGun,
    stompymux_rs::BattleWeapon::ClanLightMachineGun,
    stompymux_rs::BattleWeapon::ClanHeavyMachineGun,
    stompymux_rs::BattleWeapon::ClanLrm5,
    stompymux_rs::BattleWeapon::ClanLrm10,
    stompymux_rs::BattleWeapon::ClanLrm15,
    stompymux_rs::BattleWeapon::ClanLrm20,
    stompymux_rs::BattleWeapon::ClanSrm2,
    stompymux_rs::BattleWeapon::ClanSrm4,
    stompymux_rs::BattleWeapon::ClanSrm6,
    stompymux_rs::BattleWeapon::ClanStreakSrm2,
    stompymux_rs::BattleWeapon::ClanStreakSrm4,
    stompymux_rs::BattleWeapon::ClanStreakSrm6,
    stompymux_rs::BattleWeapon::PlasmaRifle,
    stompymux_rs::BattleWeapon::AcidThrower,
    stompymux_rs::BattleWeapon::Flamer,
    stompymux_rs::BattleWeapon::MachineGun,
    stompymux_rs::BattleWeapon::HeavyMachineGun,
    stompymux_rs::BattleWeapon::LightAc2,
    stompymux_rs::BattleWeapon::LightAc5,
    stompymux_rs::BattleWeapon::UltraAc2,
    stompymux_rs::BattleWeapon::UltraAc5,
    stompymux_rs::BattleWeapon::UltraAc10,
    stompymux_rs::BattleWeapon::UltraAc20,
    stompymux_rs::BattleWeapon::RotaryAc2,
    stompymux_rs::BattleWeapon::RotaryAc5,
    stompymux_rs::BattleWeapon::ClanRotaryAc2,
    stompymux_rs::BattleWeapon::ClanRotaryAc5,
    stompymux_rs::BattleWeapon::ClanRotaryAc10,
    stompymux_rs::BattleWeapon::SmallLaser,
    stompymux_rs::BattleWeapon::LargeLaser,
    stompymux_rs::BattleWeapon::Ppc,
    stompymux_rs::BattleWeapon::ErSmallLaser,
    stompymux_rs::BattleWeapon::ErMediumLaser,
    stompymux_rs::BattleWeapon::ErLargeLaser,
    stompymux_rs::BattleWeapon::ErPpc,
    stompymux_rs::BattleWeapon::SmallPulseLaser,
    stompymux_rs::BattleWeapon::MediumPulseLaser,
    stompymux_rs::BattleWeapon::LargePulseLaser,
    stompymux_rs::BattleWeapon::XSmallPulseLaser,
    stompymux_rs::BattleWeapon::XMediumPulseLaser,
    stompymux_rs::BattleWeapon::XLargePulseLaser,
    stompymux_rs::BattleWeapon::LightPpc,
    stompymux_rs::BattleWeapon::HeavyPpc,
    stompymux_rs::BattleWeapon::SnubNosedPpc,
    stompymux_rs::BattleWeapon::HeavyGaussRifle,
    stompymux_rs::BattleWeapon::GaussRifle,
    stompymux_rs::BattleWeapon::LightGaussRifle,
    stompymux_rs::BattleWeapon::MagshotGaussRifle,
    stompymux_rs::BattleWeapon::Ac2,
    stompymux_rs::BattleWeapon::Ac5,
    stompymux_rs::BattleWeapon::Ac10,
    stompymux_rs::BattleWeapon::Srm2,
    stompymux_rs::BattleWeapon::Mrm10,
    stompymux_rs::BattleWeapon::Mrm20,
    stompymux_rs::BattleWeapon::Mrm30,
    stompymux_rs::BattleWeapon::Mrm40,
    stompymux_rs::BattleWeapon::LrDfm5,
    stompymux_rs::BattleWeapon::LrDfm10,
    stompymux_rs::BattleWeapon::LrDfm15,
    stompymux_rs::BattleWeapon::LrDfm20,
    stompymux_rs::BattleWeapon::SrDfm2,
    stompymux_rs::BattleWeapon::SrDfm4,
    stompymux_rs::BattleWeapon::SrDfm6,
    stompymux_rs::BattleWeapon::Elrm5,
    stompymux_rs::BattleWeapon::Elrm10,
    stompymux_rs::BattleWeapon::Elrm15,
    stompymux_rs::BattleWeapon::Elrm20,
    stompymux_rs::BattleWeapon::StreakSrm2,
    stompymux_rs::BattleWeapon::StreakSrm4,
    stompymux_rs::BattleWeapon::StreakSrm6,
    stompymux_rs::BattleWeapon::Lrm5,
    stompymux_rs::BattleWeapon::Lrm10,
    stompymux_rs::BattleWeapon::Lrm15,
];

/// Conventional direct-fire weapons share native/Lua fire, durable heat and recycle handling;
/// restart probes run once per shard under the pellet-family latching convention.
async fn conventional_direct_fire_matrix(weapons: &[stompymux_rs::BattleWeapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_skill(&mut pristine, 30);
    shot_seed(&mut pristine, id, 1);
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed = false;
    for weapon in weapons.iter().copied() {
        restore_database(&config, &pristine_db);
        let mut world = pristine.clone();
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            weapon.gunnery_skill(true),
            BattleCharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let mut definition = world.btech.constructed_units()[&id].definition().clone();
        if weapon == BattleWeapon::ClanErMediumLaser {
            definition
                .attributes
                .insert("specials".into(), "Clan FlipArms".into());
            definition.heat_sinks = 20;
            for section in definition.sections.values_mut() {
                section
                    .criticals
                    .retain(|_, part| part.equipment != "HeatSink");
            }
        }
        let location = if weapon == BattleWeapon::HeavyGaussRifle {
            CriticalLocation {
                section: BattleSection::LeftTorso,
                slot: 0,
            }
        } else {
            world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons[0]
                .criticals[0]
        };
        let section = definition.sections.get_mut(&location.section).unwrap();
        let mut critical = section.criticals[&location.slot].clone();
        critical.equipment = weapon.name().into();
        for slot in location.slot..location.slot + weapon.profile().critical_slots {
            section.criticals.insert(slot, critical.clone());
        }
        let ammunition = weapon.profile().ammunition_per_ton;
        if ammunition > 0 {
            let bin = definition
                .sections
                .values_mut()
                .flat_map(|section| section.criticals.values_mut())
                .find(|critical| critical.equipment.starts_with("Ammo_"))
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = ammunition.to_string();
        }
        world
            .btech
            .rewrite_unit_record(id, |record| {
                if ammunition > 0 {
                    record["ammunition"][0] = ammunition.into();
                }
                record["definition"] = serde_json::to_value(definition).unwrap();
            })
            .unwrap();
        let weapon_index = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        let before = world.btech.clone();
        install(&native, world.clone());
        install(&lua, world);
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{weapon_index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {weapon_index} #{}", target.0),
        );
        assert!(text.contains("You fire"), "{text}");
        let report: mlua::Table = lua
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{weapon_index},{})",
                id.0, target.0
            ))
            .unwrap();
        let salvo: mlua::Table = report
            .get::<mlua::Table>("salvo")
            .unwrap()
            .get("report")
            .unwrap();
        let groups: mlua::Table = salvo.get("groups").unwrap();
        let cluster: Option<u8> = salvo.get("cluster_roll").unwrap();
        let distance = report
            .get::<mlua::Table>("aim")
            .unwrap()
            .get::<f64>("distance")
            .unwrap();
        let mut attack_dice = BattleDice::seeded([1; 32]);
        let mut values = vec![attack_dice.d6(), attack_dice.d6()];
        if weapon.is_dead_fire()
            || (matches!(
                weapon,
                BattleWeapon::Elrm5
                    | BattleWeapon::Elrm10
                    | BattleWeapon::Elrm15
                    | BattleWeapon::Elrm20
            ) && distance < 10.0)
        {
            values.push(attack_dice.d6());
            values.sort_unstable();
        }
        assert_eq!(report.get::<u8>("roll").unwrap(), values[0] + values[1]);
        let expected_groups = weapon.damage_groups_at_range(cluster, distance).unwrap();
        let delivered = groups.len().unwrap() as usize;
        assert!(delivered <= expected_groups.len());
        if delivered < expected_groups.len() {
            assert!(
                lua.world().btech.constructed_units()[&target].is_destroyed(),
                "Only destruction ends a salvo early"
            );
        }
        for (index, damage) in expected_groups.into_iter().take(delivered).enumerate() {
            assert_eq!(
                groups
                    .get::<mlua::Table>(index + 1)
                    .unwrap()
                    .get::<u16>("damage")
                    .unwrap(),
                damage
            );
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let mut fired = native.world().clone();
        let unit = &fired.btech.constructed_units()[&id];
        if ammunition > 0 {
            assert_eq!(unit.ammunition()[0], u16::from(ammunition) - 1);
        }
        assert_eq!(unit.heat().stored, f64::from(weapon.profile().heat));
        assert_eq!(
            unit.weapon_recycle()[&weapon_index],
            u16::from(weapon.profile().recycle_seconds)
        );
        // Restart probe runs once per shard; every weapon keeps its recycle notices.
        let mut restored = if probed {
            None
        } else {
            probed = true;
            persistence::save(&config.database(), &fired).await.unwrap();
            Some(persistence::load(&config.database()).await.unwrap())
        };
        for tick in 1..=weapon.profile().recycle_seconds {
            let notices = advance_battle_recycle(&mut fired);
            if let Some(restored) = restored.as_mut() {
                assert_eq!(advance_battle_recycle(restored), notices);
                assert_eq!(restored.btech, fired.btech);
            }
            assert_eq!(
                notices.len(),
                usize::from(tick == weapon.profile().recycle_seconds)
            );
        }
        assert!(
            fired.btech.constructed_units()[&id]
                .weapon_recycle()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_01() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[0..12]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_02() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[12..24]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_03() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[24..36]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_04() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[36..48]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_05() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[48..60]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_06() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[60..72]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_07() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[72..84]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_08() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[84..96]).await;
}

#[tokio::test]
async fn conventional_direct_fire_parity_damage_and_recycle_survive_restart_09() {
    conventional_direct_fire_matrix(&CONVENTIONAL_DIRECT_FIRE_WEAPONS[96..]).await;
}

/// Heat-mode shots transfer full heat, consume no target dice and share native/Lua transactions.
#[tokio::test]
async fn flamer_heat_mode_native_lua_persistence_and_recycle_guards() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 30);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    definition
        .sections
        .get_mut(&BattleSection::LeftArm)
        .unwrap()
        .criticals
        .get_mut(&2)
        .unwrap()
        .equipment = "IS.Flamer".into();
    state["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(definition).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    assert!(toggle_battle_flamer_heat(&mut world, id, ObjectId(1), 1).is_err());
    world.validate(&config).unwrap();
    for (index, mode) in [(1, "heat"), (999, "heat"), (0, "normal")] {
        let mut corrupt = world.clone();
        corrupt
            .btech
            .rewrite_unit_record(id, |record| {
                record["fire_modes"][index.to_string()] = mode.into();
            })
            .unwrap();
        assert!(corrupt.validate(&config).is_err());
    }
    let before = world.btech.clone();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.flamerheat({},1,0); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let text = support::run_text(&native, &config, ObjectId(1), 1, "flamerheat 0");
    assert!(
        text.contains("Weapon 0 has been set to HEAT mode"),
        "{text}"
    );
    let mode: String = lua
        .eval_callback(&format!("return btech.unit.flamerheat({},1,0)", id.0))
        .unwrap();
    assert_eq!(mode, "heat");
    assert_eq!(native.world().btech, lua.world().btech);
    let mode: String = lua
        .eval_callback(&format!(
            "return btech.unit.weapon_states({})[1].fire_mode",
            id.0
        ))
        .unwrap();
    assert_eq!(mode, "heat");
    assert!(support::run_text(&native, &config, ObjectId(1), 1, "weapons").contains("[HEAT]"));
    let target_before =
        serde_json::to_value(&lua.world().btech.constructed_units()[&target]).unwrap();
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    assert!(text.contains("Hit."), "{text}");
    let report: mlua::Table = lua
        .eval_callback(&format!(
            "return btech.unit.fire({},1,0,{})",
            id.0, target.0
        ))
        .unwrap();
    assert_eq!(report.get::<u8>("heat_transfer").unwrap(), 2);
    assert!(matches!(
        report.get::<mlua::Value>("salvo").unwrap(),
        mlua::Value::Nil
    ));
    assert_eq!(native.world().btech, lua.world().btech);
    let mut expected_target = target_before;
    expected_target["heat"]["stored"] = serde_json::json!(2.0);
    assert_eq!(
        serde_json::to_value(&lua.world().btech.constructed_units()[&target]).unwrap(),
        expected_target
    );
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<String>(&format!("return btech.unit.flamerheat({},1,0)", id.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let mut saved = lua.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.constructed_units()[&id]
            .fire_mode(0)
            .unwrap(),
        BattleFireMode::Heat
    );
    for _ in 0..10 {
        assert_eq!(
            advance_battle_recycle(&mut saved),
            advance_battle_recycle(&mut restored)
        );
    }
    assert_eq!(
        toggle_battle_flamer_heat(&mut restored, id, ObjectId(1), 0).unwrap(),
        BattleFireMode::Normal
    );
}

/// Glancing reduces armor damage, but the flamer's heat transfer uses its base damage.
#[tokio::test]
async fn flamer_heat_hits_glances_and_misses_do_not_roll_target_damage() {
    use stompymux_rs::*;
    let (_dir, _config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 4);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Ballistic",
        BattleCharacterValue {
            value: 4,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    for weapon in [
        BattleWeapon::ClanFlamer,
        BattleWeapon::Flamer,
        BattleWeapon::HeavyFlamer,
        BattleWeapon::VehicleFlamer,
        BattleWeapon::VehicleHeavyFlamer,
    ] {
        let mut base = base.clone();
        let mut definition = base.btech.constructed_units()[&id].definition().clone();
        definition
            .sections
            .get_mut(&BattleSection::LeftArm)
            .unwrap()
            .criticals
            .get_mut(&2)
            .unwrap()
            .equipment = weapon.name().into();
        if weapon.profile().ammunition_per_ton > 0 {
            let bin = definition
                .sections
                .get_mut(&BattleSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
        }
        base.btech
            .rewrite_unit_record(id, |record| {
                if weapon.profile().ammunition_per_ton > 0 {
                    record["ammunition"][0] = weapon.profile().ammunition_per_ton.into();
                }
                record["definition"] = serde_json::to_value(definition).unwrap();
            })
            .unwrap();
        toggle_battle_flamer_heat(&mut base, id, ObjectId(1), 0).unwrap();
        let rules = BattleShotRules {
            range_damage: false,
            tsm_tow_bonus: true,
            glancing: BattleGlancingMode::AtTarget,
            ..shot_rules()
        };
        let mut probe = base.clone();
        let target_number = resolve_battle_shot(&mut probe, id, ObjectId(1), target, 0, rules)
            .unwrap()
            .target_number
            .unwrap();
        assert!((3..=11).contains(&target_number), "{target_number}");
        for roll in [target_number - 1, target_number, target_number + 1] {
            let mut world = base.clone();
            let seed = (0..=u8::MAX)
                .find(|seed| i32::from(BattleDice::seeded([*seed; 32]).two_d6()) == roll)
                .unwrap();
            shot_seed(&mut world, id, seed);
            let target_before = &base.btech.constructed_units()[&target];
            let report =
                resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
            assert_eq!(
                report.heat_transfer,
                if roll >= target_number {
                    weapon.profile().damage
                } else {
                    0
                }
            );
            assert_eq!(report.glancing, roll == target_number);
            assert!(report.salvo.is_none());
            if weapon.profile().ammunition_per_ton > 0 {
                assert_eq!(
                    world.btech.constructed_units()[&id].ammunition()[0],
                    u16::from(weapon.profile().ammunition_per_ton) - 1
                );
            }
            let after = &world.btech.constructed_units()[&target];
            assert_eq!(after.sections(), target_before.sections());
            assert_eq!(
                serde_json::to_value(after).unwrap()["dice"],
                serde_json::to_value(target_before).unwrap()["dice"]
            );
        }
    }
}

/// Selection clauses commit in order, while delivery failures undo the whole command.
#[tokio::test]
async fn flamer_selections_preserve_order_partial_errors_and_transaction_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, _) = shot_fixture().await;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    for mount in &loadout.weapons[..2] {
        assert_eq!(mount.criticals.len(), 1);
        let location = mount.criticals[0];
        definition
            .sections
            .get_mut(&location.section)
            .unwrap()
            .criticals
            .get_mut(&location.slot)
            .unwrap()
            .equipment = "IS.Flamer".into();
    }
    world.btech.set_unit_definition(id, definition).unwrap();
    world.validate(&config).unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let text = support::run_text(&native, &config, ObjectId(1), 1, "flamerheat 2-0,0");
    let notices: Vec<_> = text.lines().collect();
    assert_eq!(
        notices,
        [
            "Weapon 0 has been set to HEAT mode",
            "Weapon 1 has been set to HEAT mode",
            "That weapon cannot be set HEAT!",
            "Weapon 0 has been set to normal mode",
        ]
    );
    assert_eq!(
        native.world().btech.constructed_units()[&id]
            .fire_mode(0)
            .unwrap(),
        BattleFireMode::Normal
    );
    assert_eq!(
        native.world().btech.constructed_units()[&id]
            .fire_mode(1)
            .unwrap(),
        BattleFireMode::Heat
    );
    for suffix in ["99,1", "x,1", "0-95,1", ",1"] {
        *native.world_mut() = world.clone();
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("flamerheat 0,{suffix}"),
        );
        assert!(
            text.starts_with("Weapon 0 has been set to HEAT mode"),
            "{text}"
        );
        assert_eq!(
            native.world().btech.constructed_units()[&id]
                .fire_mode(0)
                .unwrap(),
            BattleFireMode::Heat
        );
        assert_eq!(
            native.world().btech.constructed_units()[&id]
                .fire_mode(1)
                .unwrap(),
            BattleFireMode::Normal
        );
    }
    *native.world_mut() = world.clone();
    let text = support::run_text(&native, &config, ObjectId(1), 1, "flamerheat 0,0");
    assert!(text.contains("normal mode"));
    assert_eq!(native.world().btech, world.btech);

    let path = _dir.path().join("stompymux.toml");
    let mut source: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    source["lua"]
        .as_table_mut()
        .unwrap()
        .insert("output_byte_limit".into(), toml::Value::Integer(50));
    std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
    let limited = Config::load(_dir.path()).unwrap();
    let native = Scripts::new(
        &limited,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let text = support::run_text(&native, &limited, ObjectId(1), 1, "flamerheat 0");
    assert!(text.contains("set to HEAT"), "{text}");
    *native.world_mut() = world.clone();
    let text = support::run_text(&native, &limited, ObjectId(1), 1, "flamerheat 0,1");
    assert!(text.contains("output limit"), "{text}");
    assert!(!text.contains("set to HEAT"), "{text}");
    assert_eq!(native.world().btech, world.btech);
}

/// Pulse accuracy changes the hit boundary without altering range or damage grouping.
#[tokio::test]
async fn pulse_accuracy_changes_a_miss_to_a_glancing_hit() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 4);
    let mut variants = Vec::new();
    for weapon in [BattleWeapon::SmallLaser, BattleWeapon::SmallPulseLaser] {
        let mut world = base.clone();
        let mut definition = world.btech.constructed_units()[&id].definition().clone();
        definition
            .sections
            .get_mut(&BattleSection::LeftArm)
            .unwrap()
            .criticals
            .get_mut(&2)
            .unwrap()
            .equipment = weapon.name().into();
        world.btech.set_unit_definition(id, definition).unwrap();
        world.validate(&config).unwrap();
        variants.push(world);
    }
    let ordinary =
        battle_aim_modifiers(&variants[0], id, target, 0, 7, optical_aim_rules()).unwrap();
    let pulse = battle_aim_modifiers(&variants[1], id, target, 0, 7, optical_aim_rules()).unwrap();
    assert_eq!(ordinary.range, pulse.range);
    assert_eq!(ordinary.weapon_accuracy, 0);
    assert_eq!(pulse.weapon_accuracy, -2);
    assert_eq!(
        pulse.subtotal(),
        ordinary.subtotal().map(|target| target - 2)
    );
    let rules = BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        glancing: BattleGlancingMode::AtTarget,
        ..shot_rules()
    };
    let mut probe = variants[1].clone();
    let threshold = resolve_battle_shot(&mut probe, id, ObjectId(1), target, 0, rules)
        .unwrap()
        .target_number
        .unwrap();
    assert!((2..=12).contains(&threshold));
    let seed = (0..=u8::MAX)
        .find(|seed| i32::from(BattleDice::seeded([*seed; 32]).two_d6()) == threshold)
        .unwrap();
    for (index, mut world) in variants.into_iter().enumerate() {
        shot_seed(&mut world, id, seed);
        let report = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
        assert_eq!(report.roll, threshold as u8);
        assert_eq!(report.glancing, index == 1);
        if index == 0 {
            assert!(report.salvo.is_none());
            continue;
        }
        assert_eq!(report.aim.weapon_accuracy, -2);
        let salvo = report.salvo.unwrap().into_mech().unwrap();
        assert_eq!(salvo.groups.len(), 1);
        assert_eq!(salvo.groups[0].damage, 2);
    }
}

/// Snub damage follows live fractional range through native/Lua fire and restart.
#[tokio::test]
async fn snub_ppc_range_damage_native_lua_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    let map = base.create(&config, "Long energy field".into(), Kind::Room);
    let source = format!("3 32\n{}", ".0.0.0\n".repeat(32));
    create_battle_map(
        &mut base,
        map,
        "snub.map",
        MapAsset::from_cells(&source).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
    stop_battle_unit(&mut base, id, ObjectId(1), fall_rules()).unwrap();
    place_battle_unit(&mut base, id, map, 1, 25).unwrap();
    assign_battle_pilot(&mut base, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut base, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut base, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut base, 0);
    }
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    let arm = definition
        .sections
        .get_mut(&BattleSection::LeftArm)
        .unwrap();
    for slot in [2, 3] {
        arm.criticals.get_mut(&slot).unwrap().equipment = "IS.SnubNosedPPC".into();
    }
    base.btech.set_unit_definition(id, definition).unwrap();
    // Fix scanner and damage streams so range assertions cannot fail on random contact loss.
    shot_seed(&mut base, id, 1);
    shot_seed(&mut base, target, 1);
    for (distance, damage) in [
        (9.0_f64, 10_u16),
        (9.001, 8),
        (13.0, 8),
        (13.001, 5),
        (15.0, 5),
    ] {
        let mut world = base.clone();
        place_battle_unit(&mut world, target, map, 1, (25.0 - distance).round() as i64).unwrap();
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["motion"]["point"]["y"] = (25.0 - distance).into();
            })
            .unwrap();
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        // Connection state is re-established by login, not stored in the world database.
        restored
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert!(text.contains("You fire"), "{distance}: {text}");
        let report: mlua::Table = lua
            .eval_callback(&format!(
                "return btech.unit.fire({},1,0,{})",
                id.0, target.0
            ))
            .unwrap();
        let aim: mlua::Table = report.get("aim").unwrap();
        assert!((aim.get::<f64>("distance").unwrap() - distance).abs() < 1e-9);
        let salvo: mlua::Table = report
            .get::<mlua::Table>("salvo")
            .unwrap()
            .get("report")
            .unwrap();
        let groups: mlua::Table = salvo.get("groups").unwrap();
        assert_eq!(groups.len().unwrap(), 1);
        assert_eq!(
            groups
                .get::<mlua::Table>(1)
                .unwrap()
                .get::<u16>("damage")
                .unwrap(),
            damage
        );
        assert_eq!(native.world().btech, lua.world().btech);
    }
}

/// Double sinks group three slots while template counts already represent cooling capacity.
#[tokio::test]
async fn double_heat_sinks_group_damage_mass_cooling_and_restart() {
    use stompymux_rs::*;
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let sink = template
        .sections
        .get_mut(&BattleSection::Head)
        .unwrap()
        .criticals
        .remove(&3)
        .unwrap();
    template
        .attributes
        .insert("specials".into(), "doublehs FlipArms".into());
    template.heat_sinks = 24;
    for slot in 2..8 {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(slot, sink.clone());
    }
    let mut incomplete = template.clone();
    incomplete
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap()
        .criticals
        .remove(&4);
    assert!(BattleLoadout::resolve(&incomplete).is_err());
    let mut split = template.clone();
    split
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap()
        .criticals
        .remove(&4);
    split
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .insert(4, sink);
    assert!(BattleLoadout::resolve(&split).is_err());
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    world.validate(&config).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .dissipation,
        24.0
    );
    let initial_mass = world.btech.constructed_units()[&id]
        .mass()
        .unwrap()
        .equipment;
    let baseline = world.clone();
    for slot in 2..5 {
        let mut damaged = baseline.clone();
        let location = CriticalLocation {
            section: BattleSection::LeftTorso,
            slot,
        };
        assert!(
            destroy_battle_critical(&mut damaged, id, location)
                .unwrap()
                .is_some()
        );
        let unit = &damaged.btech.constructed_units()[&id];
        assert_eq!(unit.system_hits(BattleSystem::HeatSink), 2);
        assert_eq!(unit.heat_rates(&damaged).dissipation, 22.0);
        assert_eq!(unit.mass().unwrap().equipment, initial_mass - 1023);
        for slot in 2..5 {
            assert!(unit.critical_destroyed(CriticalLocation {
                section: BattleSection::LeftTorso,
                slot
            }));
        }
        assert!(
            destroy_battle_critical(&mut damaged, id, location)
                .unwrap()
                .is_none()
        );
        damaged.validate(&config).unwrap();
        world = damaged;
    }
    let mut malformed = serde_json::to_value(&world.btech).unwrap();
    malformed["constructed"][id.0.to_string()]["lost_criticals"]
        .as_array_mut()
        .unwrap()
        .pop();
    let mut corrupt = world.clone();
    corrupt.btech = serde_json::from_value(malformed).unwrap();
    assert!(corrupt.validate(&config).is_err());
    // Both installed groups stop cooling when their containing section floods.
    let mut flooded = serde_json::to_value(&baseline.btech).unwrap();
    flooded["constructed"][id.0.to_string()]["flooded_sections"] = serde_json::json!(["LeftTorso"]);
    let mut wet = baseline.clone();
    wet.btech = serde_json::from_value(flooded).unwrap();
    wet.validate(&config).unwrap();
    assert_eq!(
        wet.btech.constructed_units()[&id].system_hits(BattleSystem::HeatSink),
        4
    );
    assert_eq!(
        wet.btech.constructed_units()[&id]
            .heat_rates(&wet)
            .dissipation,
        20.0
    );
    spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..10 {
        assert_eq!(
            advance_battle_heat(&mut restored),
            advance_battle_heat(&mut world)
        );
        assert_eq!(restored.btech, world.btech);
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let lost: usize = scripts
        .eval_callback(&format!(
            "return #btech.unit.state({}).lost_criticals",
            id.0
        ))
        .unwrap();
    assert_eq!(lost, 3);
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "fliparms");
    assert!(text.contains("flipped"), "{text}");
}

/// Fusion families retain slot-derived identity through hits, shutdown and restart.
#[tokio::test]
async fn fusion_engine_layout_mass_damage_and_restart() {
    use stompymux_rs::*;
    for (engine, sides, compact, mass, spelling) in [
        (BattleEngine::Standard, 0, false, 9216, "standard"),
        (BattleEngine::Light, 2, false, 7168, "light"),
        (BattleEngine::Xl, 3, false, 4608, "xl"),
        (BattleEngine::Xxl, 6, false, 3072, "xxl"),
        (BattleEngine::Compact, 0, true, 13824, "compact"),
    ] {
        let mut template =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        template.max_speed = 96.75;
        let part = template.sections[&BattleSection::CenterTorso].criticals[&0].clone();
        if compact {
            for slot in 7..10 {
                template
                    .sections
                    .get_mut(&BattleSection::CenterTorso)
                    .unwrap()
                    .criticals
                    .remove(&slot);
            }
        }
        for section in [BattleSection::LeftTorso, BattleSection::RightTorso] {
            let layout = template.sections.get_mut(&section).unwrap();
            let vacant: Vec<_> = (0..12)
                .filter(|slot| !layout.criticals.contains_key(slot))
                .take(sides)
                .collect();
            for slot in vacant {
                layout.criticals.insert(slot, part.clone());
            }
        }
        let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
        let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
        assert_eq!(
            world.btech.constructed_units()[&id].engine().unwrap(),
            engine
        );
        assert_eq!(
            world.btech.constructed_units()[&id].mass().unwrap().engine,
            mass,
            "{spelling}"
        );
        world.validate(&config).unwrap();
        if sides > 0 {
            let slot = template.sections[&BattleSection::LeftTorso]
                .criticals
                .iter()
                .find(|(_, part)| part.equipment == "Engine")
                .unwrap()
                .0
                .to_owned();
            template
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .remove(&slot);
            assert!(BattleUnit::from_template(template).is_err());
        }
        let mut collapsed = world.clone();
        apply_damage_phase(
            &mut collapsed,
            id,
            BattleSection::LeftTorso,
            100,
            BattleDamagePhase::Internal,
        )
        .unwrap();
        let unit = &collapsed.btech.constructed_units()[&id];
        assert_eq!(unit.is_destroyed(), sides >= 3, "{spelling}");
        assert_eq!(unit.engine().unwrap(), engine);
        if sides == 2 {
            assert_eq!(unit.heat_rates(&collapsed).production, 10.0);
        }
        collapsed.validate(&config).unwrap();
        let criticals: Vec<_> = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .systems
            .into_iter()
            .filter(|part| part.system == BattleSystem::Engine)
            .map(|part| part.location)
            .collect();
        for (index, location) in criticals.iter().take(2).enumerate() {
            destroy_battle_critical(&mut world, id, *location).unwrap();
            let unit = &world.btech.constructed_units()[&id];
            assert!(!unit.is_destroyed());
            assert_eq!(unit.heat_rates(&world).production, (index + 1) as f64 * 5.0);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, restored.btech);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let identity: String = lua
            .eval_callback(&format!("return btech.unit.state({}).engine", id.0))
            .unwrap();
        assert_eq!(identity, spelling);
        support::run_text(&native, &config, ObjectId(1), 1, "shutdown");
        lua.eval_callback::<()>(&format!("btech.unit.stop({},1)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        destroy_battle_critical(&mut world, id, criticals[2]).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.is_destroyed());
        assert_eq!(unit.power(), BattlePower::Off);
        assert_eq!(unit.pilot(), None);
        assert_eq!(unit.engine().unwrap(), engine);
        world.validate(&config).unwrap();
    }
}

/// CASE contains the explosion without preventing local destruction, injury or later weapon transfer.
#[tokio::test]
async fn case_contains_ammunition_but_not_weapon_damage_or_xl_loss() {
    use stompymux_rs::*;
    let (_dir, config, mut baseline, id) = fixture('.').await;
    shot_skill(&mut baseline, 20);
    shot_seed(&mut baseline, id, 1);
    let original_mass = baseline.btech.constructed_units()[&id].mass().unwrap();
    let original_candidates =
        baseline.btech.constructed_units()[&id].critical_candidates(BattleSection::RightTorso);
    let mut definition = baseline.btech.constructed_units()[&id].definition().clone();
    let mut part = definition.sections[&BattleSection::RightTorso].criticals[&1].clone();
    part.equipment = "CASE".into();
    definition
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .configuration = Some("Case".into());
    definition
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .insert(3, part);
    let mut state = serde_json::to_value(&baseline.btech).unwrap();
    state["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(&definition).unwrap();
    let mut protected = baseline.clone();
    protected.btech = serde_json::from_value(state).unwrap();
    let unit = &protected.btech.constructed_units()[&id];
    assert!(unit.has_case(BattleSection::RightTorso));
    assert!(!unit.has_case(BattleSection::CenterTorso));
    assert_eq!(
        unit.critical_candidates(BattleSection::RightTorso),
        original_candidates
    );
    assert_eq!(
        unit.mass().unwrap().equipment,
        original_mass.equipment + 512
    );
    protected.validate(&config).unwrap();
    let before = protected.clone();
    persistence::save(&config.database(), &protected)
        .await
        .unwrap();
    let report = explode_battle_ammunition(&mut protected, id, 0, fall_rules()).unwrap();
    assert!(!report.impact.destroyed);
    assert_eq!(report.pilot_injuries.last().unwrap().injuries, 2);
    let unit = &protected.btech.constructed_units()[&id];
    assert_eq!(unit.sections()[&BattleSection::RightTorso].internal, 0);
    assert_eq!(unit.sections()[&BattleSection::RightArm].internal, 0);
    assert_eq!(
        unit.sections()[&BattleSection::CenterTorso],
        before.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso]
    );
    assert_eq!(unit.ammunition(), &[0]);
    assert!(unit.has_case(BattleSection::RightTorso));
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        explode_battle_ammunition(&mut restored, id, 0, fall_rules()).unwrap(),
        report
    );
    assert_eq!(restored.btech, protected.btech);
    assert!(explode_battle_ammunition(&mut protected, id, 0, fall_rules()).is_err());
    assert_eq!(restored.btech, protected.btech);
    let uncontained = explode_battle_ammunition(&mut baseline, id, 0, fall_rules()).unwrap();
    assert!(uncontained.impact.destroyed);
    // A weapon packet hitting the ruined CASE section still transfers into center armor.
    resolve_battle_tactical_impact(
        &mut protected,
        id,
        BattleHit {
            section: BattleSection::RightTorso,
            rear_armor: false,
            through_armor_critical: false,
            crew_stun: false,
        },
        40,
        fall_rules(),
    )
    .unwrap();
    assert_eq!(
        protected.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].armor,
        0
    );
    let engine = definition.sections[&BattleSection::CenterTorso].criticals[&0].clone();
    for (section, start) in [
        (BattleSection::LeftTorso, 2),
        (BattleSection::RightTorso, 4),
    ] {
        for slot in start..start + 3 {
            definition
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(slot, engine.clone());
        }
    }
    let mut xl = before;
    xl.btech.set_unit_definition(id, definition).unwrap();
    let center = xl.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso].clone();
    let report = explode_battle_ammunition(&mut xl, id, 0, fall_rules()).unwrap();
    assert!(report.impact.destroyed);
    assert_eq!(
        xl.btech.constructed_units()[&id].sections()[&BattleSection::CenterTorso],
        center
    );
    xl.validate(&config).unwrap();
}

/// Native and Lua shots share CASE containment, including callback rollback of nested explosions.
#[tokio::test]
async fn case_weapon_triggered_explosion_native_lua_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    shot_seed(&mut base, id, 1);
    let mut definition = base.btech.constructed_units()[&target].definition().clone();
    let mut part = definition.sections[&BattleSection::RightTorso].criticals[&1].clone();
    part.equipment = "Case".into();
    definition
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .insert(3, part);
    base.btech
        .rewrite_unit_record(target, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["sections"]["RightTorso"]["armor"] = 0.into();
        })
        .unwrap();
    for value in 0..=u8::MAX {
        let mut before = base.clone();
        shot_seed(&mut before, target, value);
        let mut trial = before.clone();
        let report = resolve_battle_shot(
            &mut trial,
            id,
            ObjectId(1),
            target,
            0,
            configured_shot_rules(&config),
        )
        .unwrap();
        if !report
            .notices()
            .iter()
            .any(|notice| notice.text == "Ammunition explosion!")
        {
            continue;
        }
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
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,0,{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert!(text.contains("You fire"), "{text}");
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,0,{})",
            id.0, target.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let world = native.world();
        let unit = &world.btech.constructed_units()[&target];
        assert!(!unit.is_destroyed());
        assert_eq!(unit.ammunition(), &[0]);
        assert_eq!(unit.sections()[&BattleSection::RightTorso].internal, 0);
        // Only the triggering laser's remaining packet may pass the destroyed CASE section.
        assert!(unit.sections()[&BattleSection::CenterTorso].internal > 0);
        return;
    }
    panic!("No seeded shot triggered the ammunition bin");
}

/// Streak lock failures recycle without launching; glancing rules cannot weaken a successful lock.
#[tokio::test]
async fn streak_lock_failure_launch_boundaries_and_saved_recycle() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 4);
    shot_seed(&mut base, target, 1);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Missile",
        BattleCharacterValue {
            value: 4,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::Srm4)
        .unwrap();
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    for part in definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
    {
        part.equipment = part.equipment.replace("IS.SRM-4", "IS.StreakSRM-4");
    }
    base.btech.set_unit_definition(id, definition).unwrap();
    let mut probe = base.clone();
    let threshold = resolve_battle_shot(&mut probe, id, ObjectId(1), target, index, shot_rules())
        .unwrap()
        .target_number
        .unwrap();
    assert!((3..=12).contains(&threshold));
    let seed_for = |roll| {
        (0..=u8::MAX)
            .find(|seed| i32::from(BattleDice::seeded([*seed; 32]).two_d6()) == roll)
            .unwrap()
    };
    for glancing in [
        BattleGlancingMode::Disabled,
        BattleGlancingMode::AtTarget,
        BattleGlancingMode::BelowTarget,
    ] {
        for roll in [threshold - 1, threshold] {
            let mut world = base.clone();
            shot_seed(&mut world, id, seed_for(roll));
            let report = resolve_battle_shot(
                &mut world,
                id,
                ObjectId(1),
                target,
                index,
                BattleShotRules {
                    range_damage: false,
                    tsm_tow_bonus: true,
                    glancing,
                    ..shot_rules()
                },
            )
            .unwrap();
            assert_eq!(report.launched, roll == threshold);
            assert_eq!(
                world.btech.constructed_units()[&id].fired_recently(),
                report.launched
            );
            assert!(!report.glancing);
            assert_eq!(
                world.btech.constructed_units()[&id].weapon_recycle()[&index],
                15
            );
            if roll < threshold {
                assert_eq!(report.expenditure.heat, 0);
                assert!(report.expenditure.ammunition.is_empty());
                assert!(report.salvo.is_none());
                assert_eq!(
                    world.btech.constructed_units()[&target],
                    base.btech.constructed_units()[&target]
                );
                assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[25]);
                assert_eq!(
                    world.btech.constructed_units()[&id].heat(),
                    base.btech.constructed_units()[&id].heat()
                );
                continue;
            }
            let salvo = report.salvo.unwrap().into_mech().unwrap();
            assert_eq!(salvo.groups.len(), 4);
            assert!(salvo.groups.iter().all(|group| group.damage == 2));
            assert_eq!(report.expenditure.heat, 3);
            assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[24]);
        }
    }
    shot_seed(&mut base, id, seed_for(threshold - 1));
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
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{},{}); error('abort')",
            id.0, index, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire {index} #{}", target.0),
    );
    assert!(text.contains("Your streak fails to lock on."), "{text}");
    assert!(!text.contains("has fired"), "{text}");
    let report: mlua::Table = lua
        .eval_callback(&format!(
            "return btech.unit.fire({},1,{}, {})",
            id.0, index, target.0
        ))
        .unwrap();
    assert!(!report.get::<bool>("launched").unwrap());
    assert_eq!(native.world().btech, lua.world().btech);
    let mut saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for tick in 1..=15 {
        let notices = advance_battle_recycle(&mut saved);
        assert_eq!(advance_battle_recycle(&mut restored), notices);
        assert_eq!(restored.btech, saved.btech);
        assert_eq!(notices.is_empty(), tick < 15);
    }
    assert!(
        restored.btech.constructed_units()[&id]
            .weapon_recycle()
            .is_empty()
    );
}

/// The signed MRM accuracy penalty reaches the shared target-number calculation.
#[tokio::test]
async fn mrm_accuracy_penalty_increases_target_number() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let before = battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules()).unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for slot in [2, 3] {
        definition
            .sections
            .get_mut(&BattleSection::LeftArm)
            .unwrap()
            .criticals
            .get_mut(&slot)
            .unwrap()
            .equipment = "IS.MRM-10".into();
    }
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = "Ammo_IS.MRM-10".into();
    bin.data = "24".into();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"][0] = 24.into();
        })
        .unwrap();
    world.validate(&config).unwrap();
    let after = battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules()).unwrap();
    assert_eq!(after.weapon_accuracy, 1);
    assert_eq!(after.range, before.range);
    assert_eq!(after.subtotal(), before.subtotal().map(|number| number + 1));
}

/// Gauss critical explosions share native/Lua crew effects and atomic callback rollback.
#[tokio::test]
async fn gauss_weapon_explosion_native_lua_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    shot_seed(&mut base, id, 1);
    let mut definition = base.btech.constructed_units()[&target].definition().clone();
    let arm = definition
        .sections
        .get_mut(&BattleSection::LeftArm)
        .unwrap();
    let mut part = arm.criticals[&2].clone();
    part.equipment = "IS.GaussRifle".into();
    arm.criticals.retain(|&slot, _| slot < 2);
    for slot in 2..9 {
        arm.criticals.insert(slot, part.clone());
    }
    base.btech.set_unit_definition(target, definition).unwrap();
    for value in 0..=u8::MAX {
        let mut before = base.clone();
        shot_seed(&mut before, target, value);
        let mut trial = before.clone();
        let report = resolve_battle_shot(
            &mut trial,
            id,
            ObjectId(1),
            target,
            0,
            configured_shot_rules(&config),
        )
        .unwrap();
        if !report
            .notices()
            .iter()
            .any(|notice| notice.text == "It explodes for 20 points damage.")
        {
            continue;
        }
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
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,0,{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert!(text.contains("You fire"), "{text}");
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,0,{})",
            id.0, target.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let world = native.world().clone();
        let unit = &world.btech.constructed_units()[&target];
        assert!(unit.lost_criticals().contains(&CriticalLocation {
            section: BattleSection::LeftArm,
            slot: 2
        }));
        assert!(
            report.notices().iter().any(
                |notice| notice.text == "You take personal injury from the weapon's explosion!"
            )
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, native.world().btech);
        return;
    }
    panic!("No seeded shot triggered the Gauss weapon");
}

/// Heavy Gauss recoil uses nominal weight classes, applies on misses, and commits with the entire shot.
async fn heavy_gauss_recoil_matrix(classes: &[(u16, i32)]) {
    use stompymux_rs::*;
    let (dir, _config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    // Match the direct scenario rules instead of inheriting optional rules from the game fixture.
    let path = dir.path().join("stompymux.toml");
    let mut source: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for name in ["extendedmovemod", "glancing_blows"] {
        source["battletech"]
            .as_table_mut()
            .unwrap()
            .insert(name.into(), 0.into());
    }
    std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    // This target stream exercises a critical cascade that exposed mismatched adapter rules.
    base.btech
        .set_unit_dice(
            target,
            BattleDice::seeded([
                132, 74, 49, 4, 20, 209, 99, 178, 82, 28, 126, 55, 74, 93, 238, 238, 168, 9, 81,
                56, 101, 52, 14, 199, 89, 158, 107, 178, 62, 214, 43, 219,
            ]),
        )
        .unwrap();
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
    // Configure groups before recoil can injure the pilot. Every replay starts
    // with identical TIC state and needs no cockpit edits after the fall.
    for (group, weapon) in [(0, index), (1, 0)] {
        edit_battle_tic(
            &mut base,
            id,
            ObjectId(1),
            group,
            BattleTicEdit::Add(vec![weapon]),
        )
        .unwrap();
    }
    for (tons, modifier) in classes.iter().copied() {
        for speed in [0.0, 1.0, -1.0] {
            for succeeds in [false, true] {
                for hits in [false, true] {
                    let seed = (0..=255)
                        .find(|seed| {
                            let mut dice = BattleDice::seeded([*seed; 32]);
                            let attack = dice.two_d6();
                            if !hits {
                                expected_grass_miss_rolls(|| dice.two_d6());
                            }
                            attack < 10 && dice.two_d6() == 2
                        })
                        .unwrap();
                    let mut before = base.clone();
                    for (name, value) in [
                        ("Piloting-Biped", if succeeds { 30 } else { 0 }),
                        ("Gunnery-Ballistic", if hits { 30 } else { 0 }),
                    ] {
                        set_battle_character_value(
                            &mut before,
                            ObjectId(1),
                            name,
                            BattleCharacterValue {
                                value,
                                experience: 0,
                                last_used: 0,
                            },
                        )
                        .unwrap();
                    }
                    let mut state = serde_json::to_value(&before.btech).unwrap();
                    state["constructed"][id.0.to_string()]["definition"]["tons"] = tons.into();
                    state["units"][id.0.to_string()]["tons"] = tons.into();
                    state["constructed"][id.0.to_string()]["motion"]["speed"] = speed.into();
                    before.btech = serde_json::from_value(state).unwrap();
                    shot_seed(&mut before, id, seed);
                    before.validate(&config).unwrap();
                    let mut world = before.clone();
                    let report = resolve_battle_shot(
                        &mut world,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        shot_rules(),
                    )
                    .unwrap();
                    assert_eq!(report.salvo.is_some(), hits);
                    assert_eq!(report.recoil.is_some(), speed != 0.0);
                    let unit = &world.btech.constructed_units()[&id];
                    assert_eq!(unit.ammunition()[0], 3);
                    assert_eq!(unit.heat().stored, 2.0);
                    if speed == 0.0 {
                        let mut expected = BattleDice::seeded([seed; 32]);
                        expected.two_d6();
                        if !hits {
                            expected_grass_miss_rolls(|| expected.two_d6());
                        }
                        assert_eq!(
                            serde_json::to_value(unit).unwrap()["dice"],
                            serde_json::to_value(expected).unwrap()
                        );
                        continue;
                    }
                    let recoil = report.recoil.as_ref().unwrap();
                    assert_eq!(recoil.check.situational, modifier);
                    assert_eq!(recoil.check.roll, Some(2));
                    assert_eq!(recoil.check.success, succeeds);
                    assert_eq!(recoil.fall.is_some(), !succeeds);
                    assert_eq!(
                        unit.posture(),
                        if succeeds {
                            BattlePosture::Standing
                        } else {
                            BattlePosture::Prone
                        }
                    );
                    if tons != 35 || speed != 1.0 || succeeds || !hits {
                        continue;
                    }
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
                    let lua =
                        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored)))
                            .unwrap();
                    assert!(
                        lua.eval_callback::<()>(&format!(
                            "btech.unit.fire({},1,{index},{}); error('abort')",
                            id.0, target.0
                        ))
                        .is_err()
                    );
                    assert_eq!(lua.world().btech, before.btech);
                    // Recoil stops the whole TIC selection before the next group fires.
                    let grouped = Scripts::new(
                        &config,
                        std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
                    )
                    .unwrap();
                    let attempts: usize = grouped
                        .eval_callback(&format!(
                            "return #btech.unit.tic_fire({},1,{{0,1}},{})",
                            id.0, target.0
                        ))
                        .unwrap();
                    assert_eq!(attempts, 1);
                    assert_eq!(
                        grouped.world().btech.constructed_units()[&id].posture(),
                        BattlePosture::Prone
                    );
                    assert_eq!(grouped.world().btech, world.btech);
                    let native =
                        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(before)))
                            .unwrap();
                    let text = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("fire {index} #{}", target.0),
                    );
                    assert!(text.contains("recoil knocks you to the ground"), "{text}");
                    let result: mlua::Table = lua
                        .eval_callback(&format!(
                            "return btech.unit.fire({},1,{index},{})",
                            id.0, target.0
                        ))
                        .unwrap();
                    assert!(
                        result
                            .get::<mlua::Table>("recoil")
                            .unwrap()
                            .get::<mlua::Table>("fall")
                            .is_ok()
                    );
                    assert_eq!(native.world().btech, lua.world().btech);
                    assert_eq!(native.world().btech, world.btech);
                }
            }
        }
    }
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_35_tons() {
    heavy_gauss_recoil_matrix(&[(35, 2)]).await;
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_40_tons() {
    heavy_gauss_recoil_matrix(&[(40, 1)]).await;
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_55_tons() {
    heavy_gauss_recoil_matrix(&[(55, 1)]).await;
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_60_tons() {
    heavy_gauss_recoil_matrix(&[(60, 0)]).await;
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_75_tons() {
    heavy_gauss_recoil_matrix(&[(75, 0)]).await;
}

#[tokio::test]
async fn heavy_gauss_recoil_weight_classes_misses_and_atomic_replay_80_tons() {
    heavy_gauss_recoil_matrix(&[(80, -1)]).await;
}

/// Heavy Gauss damage uses unrounded range through native/Lua firing and restart.
#[tokio::test]
async fn heavy_gauss_range_damage_native_lua_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    let map = base.create(&config, "Long energy field".into(), Kind::Room);
    let source = format!("3 32\n{}", ".0.0.0\n".repeat(32));
    create_battle_map(
        &mut base,
        map,
        "heavy-gauss.map",
        MapAsset::from_cells(&source).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
    stop_battle_unit(&mut base, id, ObjectId(1), fall_rules()).unwrap();
    place_battle_unit(&mut base, id, map, 1, 25).unwrap();
    assign_battle_pilot(&mut base, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut base, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut base, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut base, 0);
    }
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
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Ballistic",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
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
    // Fix scanner and damage streams so range assertions cannot fail on random contact loss.
    shot_seed(&mut base, id, 1);
    shot_seed(&mut base, target, 1);
    for (distance, damage) in [
        (6.0_f64, 25_u16),
        (6.001, 20),
        (13.0, 20),
        (13.001, 10),
        (20.0, 10),
    ] {
        let mut world = base.clone();
        place_battle_unit(&mut world, target, map, 1, (25.0 - distance).round() as i64).unwrap();
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["motion"]["point"]["y"] = (25.0 - distance).into();
            })
            .unwrap();
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        // Connection state is re-established by login, not stored in the world database.
        restored
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        assert!(text.contains("You fire"), "{distance}: {text}");
        let report: mlua::Table = lua
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
        let aim: mlua::Table = report.get("aim").unwrap();
        assert!((aim.get::<f64>("distance").unwrap() - distance).abs() < 1e-9);
        let salvo: mlua::Table = report
            .get::<mlua::Table>("salvo")
            .unwrap()
            .get("report")
            .unwrap();
        let groups: mlua::Table = salvo.get("groups").unwrap();
        assert_eq!(groups.len().unwrap(), 1);
        assert_eq!(
            groups
                .get::<mlua::Table>(1)
                .unwrap()
                .get::<u16>("damage")
                .unwrap(),
            damage
        );
        assert_eq!(native.world().btech, lua.world().btech);
    }
}

/// The unchanged Cestus can fire and restart, with heat and destruction following relocated engine slots.
#[tokio::test]
async fn relocated_cestus_engine_damage_native_lua_fire_and_restart() {
    use stompymux_rs::*;
    let template =
        BattleTemplate::parse("CES-4S", include_str!("../game/mechs/CES-4S.toml")).unwrap();
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut base, id) = fixture_assets(&source, template).await;
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    let target = base.create(&config, "Cestus target".into(), Kind::Thing);
    create_battle_unit(
        &mut base,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut base, target, map, 5, 4).unwrap();
    base.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    shot_skill(&mut base, 30);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Ballistic",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    shot_seed(&mut base, id, 1);
    shot_seed(&mut base, target, 1);
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::HeavyGaussRifle)
        .unwrap();
    base.validate(&config).unwrap();
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
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index},{}); error('abort')",
            id.0, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire {index} #{}", target.0),
    );
    assert!(text.contains("You fire"), "{text}");
    lua.eval_callback::<mlua::Table>(&format!(
        "return btech.unit.fire({},1,{index},{})",
        id.0, target.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    for (section, hits) in [
        (BattleSection::LeftTorso, 2),
        (BattleSection::RightTorso, 0),
    ] {
        let mut world = native.world().clone();
        apply_damage_phase(&mut world, id, section, 100, BattleDamagePhase::Internal).unwrap();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.engine().unwrap(), BattleEngine::Light);
        assert_eq!(unit.system_hits(BattleSystem::Engine), hits);
        assert!(!unit.is_destroyed());
        assert_eq!(unit.heat_rates(&world).production, f64::from(hits * 5));
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, world.btech);
        for slot in if hits == 2 { vec![10] } else { vec![10, 11, 0] } {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::CenterTorso,
                    slot,
                },
            )
            .unwrap();
            destroy_battle_critical(
                &mut restored,
                id,
                CriticalLocation {
                    section: BattleSection::CenterTorso,
                    slot,
                },
            )
            .unwrap();
            assert_eq!(restored.btech, world.btech);
        }
        assert!(world.btech.constructed_units()[&id].is_destroyed());
        assert_eq!(
            world.btech.constructed_units()[&id].engine().unwrap(),
            BattleEngine::Light
        );
    }
}

/// A split mount has one firing index, ammunition selection and durable recycle across native/Lua adapters.
#[tokio::test]
async fn split_weapon_native_lua_fire_guards_and_recycle_restart() {
    use stompymux_rs::*;
    for weapon in [
        BattleWeapon::Ac20,
        BattleWeapon::HeavyGaussRifle,
        BattleWeapon::Lbx20,
    ] {
        let (_dir, config, mut base, id, target) = shot_fixture().await;
        shot_skill(&mut base, 30);
        set_battle_character_value(
            &mut base,
            ObjectId(1),
            "Gunnery-Ballistic",
            BattleCharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        shot_seed(&mut base, id, 1);
        shot_seed(&mut base, target, 1);
        let mut definition = base.btech.constructed_units()[&id].definition().clone();
        let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 4..12 {
            definition
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        part.equipment = "SplitCrit_Left".into();
        part.data = "Left_Arm:4".into();
        for slot in 2..2 + weapon.profile().critical_slots - 8 {
            definition
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        let bin = definition
            .sections
            .values_mut()
            .flat_map(|s| s.criticals.values_mut())
            .find(|part| part.equipment.starts_with("Ammo_"))
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = weapon.profile().ammunition_per_ton.to_string();
        base.btech
            .rewrite_unit_record(id, |record| {
                record["definition"] = serde_json::to_value(definition).unwrap();
                record["ammunition"][0] = weapon.profile().ammunition_per_ton.into();
            })
            .unwrap();
        base.validate(&config).unwrap();
        let index = base.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        let mut broken = base.clone();
        destroy_battle_critical(
            &mut broken,
            id,
            CriticalLocation {
                section: BattleSection::LeftTorso,
                slot: 2,
            },
        )
        .unwrap();
        let before = broken.btech.clone();
        assert!(
            resolve_battle_shot(&mut broken, id, ObjectId(1), target, index, shot_rules()).is_err()
        );
        assert_eq!(broken.btech, before);
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
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        assert!(text.contains("You fire"), "{text}");
        let result: mlua::Table = lua
            .eval_callback(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
        assert_eq!(result.get::<usize>("weapon_index").unwrap(), index);
        assert_eq!(native.world().btech, lua.world().btech);
        let mut fired = native.world().clone();
        let unit = &fired.btech.constructed_units()[&id];
        assert_eq!(
            unit.ammunition()[0],
            u16::from(weapon.profile().ammunition_per_ton) - 1
        );
        assert_eq!(unit.weapon_recycle().len(), 1);
        persistence::save(&config.database(), &fired).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, fired.btech);
        for _ in 0..weapon.profile().recycle_seconds {
            assert_eq!(
                advance_battle_recycle(&mut restored),
                advance_battle_recycle(&mut fired)
            );
            assert_eq!(restored.btech, fired.btech);
        }
        assert!(
            fired.btech.constructed_units()[&id]
                .weapon_recycle()
                .is_empty()
        );
    }
}
