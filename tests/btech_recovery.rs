//! Player-owned recovery cadence, cockpit guards, restart and transactional retry.
use crate::support;
use crate::support::btech_firing as firing;
use sqlx::Connection;
use stompymux_rs::{
    BattleCharacter, BattleDice, BattlePower, ObjectId, World, advance_battle_recovery,
    check_character_consciousness, persistence, set_battle_character,
};

/// A live pilot with maximum bruising and a reproducible recovery stream.
async fn fixture() -> (tempfile::TempDir, stompymux_rs::Config, World, [u8; 32]) {
    let (dir, config, mut world) = support::isolated_world().await;
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            bruise: 50,
            lethal: 0,
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
        },
    )
    .unwrap();
    let seed = (0..=255)
        .map(|byte| [byte; 32])
        .find(|seed| {
            let mut dice = BattleDice::seeded(*seed);
            dice.two_d6() < 11 && dice.two_d6() >= 11
        })
        .unwrap();
    // Seed the same explicit representation validated by the persistence loader.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"]["1"] = serde_json::json!({
        "remaining":0,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded(seed)
    });
    world.btech = serde_json::from_value(state).unwrap();
    (dir, config, world, seed)
}

#[tokio::test]
async fn recovery_preserves_dice_and_timer_across_injury_restart_and_cockpit_release() {
    let (_dir, config, mut world, _seed) = fixture().await;
    let map = world.create(&config, "Recovery map".into(), stompymux_rs::Kind::Room);
    stompymux_rs::create_battle_map(
        &mut world,
        map,
        "recovery.map",
        stompymux_rs::BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let unit = world.create(&config, "Cockpit".into(), stompymux_rs::Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    stompymux_rs::create_battle_unit(
        &mut world,
        unit,
        stompymux_rs::BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    stompymux_rs::place_battle_unit(&mut world, unit, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    stompymux_rs::assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    stompymux_rs::start_battle_unit(&mut world, unit, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        stompymux_rs::advance_battle_units(&mut world, 0);
    }
    assert_eq!(
        world.btech.constructed_units()[&unit].power(),
        BattlePower::Running
    );
    assert!(
        !check_character_consciousness(&mut world, ObjectId(1), false, false)
            .unwrap()
            .unwrap()
            .conscious
    );
    let before = world.btech.clone();
    assert!(
        check_character_consciousness(&mut world, ObjectId(1), false, false)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.btech, before);
    assert!(stompymux_rs::set_battle_speed(&mut world, unit, ObjectId(1), 10.0).is_err());
    assert!(stompymux_rs::set_battle_heading(&mut world, unit, ObjectId(1), 90.0).is_err());
    assert!(stompymux_rs::spend_battle_weapon(&mut world, unit, ObjectId(1), 0).is_err());
    stompymux_rs::release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    assert!(stompymux_rs::assign_battle_pilot(&mut world, unit, ObjectId(1)).is_err());
    for _ in 0..12 {
        assert!(advance_battle_recovery(&mut world).is_empty());
    }
    assert_eq!(world.btech.recoveries()[&ObjectId(1)].remaining, 18);
    persistence::save(&config.database(), &world).await.unwrap();
    let saved = world.btech.clone();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, saved);
    for _ in 0..17 {
        assert!(advance_battle_recovery(&mut world).is_empty());
    }
    let notices = advance_battle_recovery(&mut world);
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].player, ObjectId(1));
    assert!(notices[0].check.conscious);
    assert!(!world.btech.unconscious(ObjectId(1)));
    stompymux_rs::assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    stompymux_rs::set_battle_speed(&mut world, unit, ObjectId(1), 10.0).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_ticks_without_a_unit_and_failed_save_replays_the_same_roll() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, _seed) = fixture().await;
        check_character_consciousness(&mut world, ObjectId(1), false, false).unwrap();
        for _ in 0..29 { advance_battle_recovery(&mut world); }
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut expected = world.clone();
        stompymux_rs::advance_battle_reactor_windows(&mut expected);
        advance_battle_recovery(&mut expected);
        // A successful server heartbeat also advances the shared turn phase.
        let mut phase_state = serde_json::to_value(&expected.btech).unwrap();
        phase_state["turn_clock"] = 1.into();
        phase_state["simulation_seconds"] = 1.into();
        expected.btech = serde_json::from_value(phase_state).unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_recovery BEFORE UPDATE ON btech_character_recovery BEGIN SELECT RAISE(ABORT,'recovery failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_recovery").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if !loaded.btech.unconscious(ObjectId(1)) {
                    assert_eq!(loaded.btech, expected.btech);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(stompymux_rs::ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn failed_recovery_reschedules_and_later_attempts_use_current_health() {
    let (_dir, _config, mut world, _seed) = fixture().await;
    let seed = (0..=255)
        .map(|byte| [byte; 32])
        .find(|seed| {
            let mut dice = BattleDice::seeded(*seed);
            dice.two_d6() < 11 && dice.two_d6() >= 3
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"]["1"]["remaining"] = 1.into();
    state["recoveries"]["1"]["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let notices = advance_battle_recovery(&mut world);
    assert!(!notices[0].check.conscious);
    assert_eq!(world.btech.recoveries()[&ObjectId(1)].remaining, 30);
    let mut profile = world.btech.characters()[&ObjectId(1)];
    let before = world.btech.clone();
    profile.build = 0;
    assert!(set_battle_character(&mut world, ObjectId(1), profile).is_err());
    assert_eq!(world.btech, before);
    profile = world.btech.characters()[&ObjectId(1)];
    profile.bruise = 0;
    set_battle_character(&mut world, ObjectId(1), profile).unwrap();
    for _ in 0..29 {
        assert!(advance_battle_recovery(&mut world).is_empty());
    }
    assert!(advance_battle_recovery(&mut world)[0].check.conscious);
    assert!(!world.btech.unconscious(ObjectId(1)));
}

/// Every cockpit snapshots its existing check without drawing presentation dice.
#[tokio::test]
async fn recovery_notices_capture_cockpit_check_and_privacy_state() {
    use stompymux_rs::*;
    for source in firing::templates() {
        let (_dir, _config, base, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        for conscious in [true, false] {
            let seed = (0..=255)
                .map(|byte| [byte; 32])
                .find(|seed| (BattleDice::seeded(*seed).two_d6() >= 7) == conscious)
                .unwrap();
            let mut dice = BattleDice::seeded(seed);
            let roll = dice.two_d6();
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["recoveries"]["1"] = serde_json::json!({
                "mode":{"kind":"tactical","injuries":3}, "remaining":1,
                "pain_resistance":false, "toughness":false, "dice":BattleDice::seeded(seed)
            });
            world.btech = serde_json::from_value(state).unwrap();
            let mut foot = world.clone();
            release_battle_pilot(&mut foot, unit, ObjectId(1)).unwrap();
            let notice = advance_battle_recovery(&mut world).pop().unwrap();
            assert_eq!(notice.unit, Some(unit));
            assert_eq!(
                notice.check,
                BattleConsciousnessCheck {
                    target: 7,
                    roll,
                    conscious
                }
            );
            let detached = advance_battle_recovery(&mut foot).pop().unwrap();
            assert_eq!(detached.unit, None);
            assert_eq!(detached.check, notice.check);
            assert_eq!(
                serde_json::to_value(&world.btech.recoveries()[&ObjectId(1)]).unwrap()["dice"],
                serde_json::to_value(dice).unwrap()
            );
            assert_eq!(
                world.btech.recoveries()[&ObjectId(1)].remaining,
                if conscious { 0 } else { 30 }
            );
        }
    }
}
