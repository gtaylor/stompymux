//! Empty tactical crews retain injury and recovery without manufacturing player identities.
use crate::support;
use stompymux_rs::*;

/// Construct an unoccupied running unit with a deterministic private recovery stream.
async fn fixture(vehicle: bool) -> (tempfile::TempDir, Config, World, ObjectId, u8) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Crew test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "crew",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Empty unit".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    if vehicle {
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
    } else {
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
        )
        .unwrap();
    }
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.two_d6() < 7 && dice.two_d6() >= 10
        })
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let state = &mut saved[if vehicle { "vehicles" } else { "constructed" }][id.0.to_string()];
    state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    state["crew_recovery"]["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    (dir, config, world, id, value)
}

/// Inspect the same saved component through either construction's public adapter.
fn recovery(world: &World, id: ObjectId) -> &BattleRecovery {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return unit.crew_recovery();
    }
    world.btech.constructed_units()[&id].crew_recovery()
}

#[tokio::test]
async fn empty_crew_injury_recovery_and_restart_share_player_rules() {
    for vehicle in [false, true] {
        let (_dir, config, mut world, id, value) = fixture(vehicle).await;
        let players = world.btech.recoveries().clone();
        assert_eq!(
            battle_unit_target_movement_modifier(&world, id, 2.0, false).unwrap(),
            0
        );
        let injury = injure_battle_tactical_pilot(&mut world, id, 3, false).unwrap();
        let check = injury.consciousness.unwrap();
        assert_eq!(check.target, 7);
        assert_eq!(check.roll, BattleDice::seeded([value; 32]).two_d6());
        assert!(!check.conscious);
        assert_eq!(recovery(&world, id).remaining, 30);
        assert_eq!(
            battle_unit_target_movement_modifier(&world, id, 2.0, false).unwrap(),
            -4
        );
        for _ in 0..3 {
            advance_battle_units(&mut world, 0);
        }
        let dice = serde_json::to_value(recovery(&world, id)).unwrap()["dice"].clone();
        let injury = injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap();
        assert!(injury.consciousness.is_none());
        assert_eq!(recovery(&world, id).remaining, 27);
        assert_eq!(
            serde_json::to_value(recovery(&world, id)).unwrap()["dice"],
            dice
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 0..26 {
            assert_eq!(
                advance_battle_units(&mut world, 0),
                advance_battle_units(&mut restored, 0)
            );
        }
        let notices = advance_battle_units(&mut world, 0);
        assert_eq!(notices, advance_battle_units(&mut restored, 0));
        assert!(
            notices.iter().any(
                |notice| notice.unit == id && notice.text == "The pilot regains consciousness!"
            )
        );
        assert_eq!(recovery(&world, id).remaining, 0);
        assert_eq!(world.btech, restored.btech);
        assert_eq!(world.btech.recoveries(), &players);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn cockpit_assignment_transfers_pending_recovery_and_death_clears_empty_crew() {
    for vehicle in [false, true] {
        let (_dir, config, mut world, id, _) = fixture(vehicle).await;
        injure_battle_tactical_pilot(&mut world, id, 3, false).unwrap();
        for _ in 0..3 {
            advance_battle_units(&mut world, 0);
        }
        let mut destroyed = world.clone();
        let death = injure_battle_tactical_pilot(&mut destroyed, id, 6, false).unwrap();
        assert!(death.killed);
        assert_eq!(recovery(&destroyed, id).remaining, 0);
        assert_eq!(recovery(&destroyed, id).mode, BattleRecoveryMode::Ready);
        destroyed.validate(&config).unwrap();
        let owned = recovery(&world, id).clone();
        prepare_battle_recovery(&mut world, ObjectId(1)).unwrap();
        let parked =
            serde_json::to_value(&world.btech.recoveries()[&ObjectId(1)]).unwrap()["dice"].clone();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        assert_eq!(world.btech.recoveries()[&ObjectId(1)], owned);
        assert_eq!(
            serde_json::to_value(recovery(&world, id)).unwrap()["dice"],
            parked
        );
        assert_eq!(recovery(&world, id).mode, BattleRecoveryMode::Ready);
        assert!(world.btech.unconscious(ObjectId(1)));
        assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
        advance_battle_units(&mut world, 0);
        assert_eq!(world.btech.recoveries()[&ObjectId(1)].remaining, 27);
        for _ in 0..27 {
            advance_battle_recovery(&mut world);
        }
        assert!(!world.btech.unconscious(ObjectId(1)));
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn empty_crew_snapshots_reject_invalid_health_and_timer_ownership() {
    for vehicle in [false, true] {
        let (_dir, config, world, id, _) = fixture(vehicle).await;
        let mut character_empty = world.clone();
        character_empty
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        assert!(injure_battle_tactical_pilot(&mut character_empty, id, 1, false).is_ok());
        assert_eq!(character_empty.btech.characters(), world.btech.characters());
        character_empty.validate(&config).unwrap();
        let class = if vehicle { "vehicles" } else { "constructed" };
        for invalid in [
            serde_json::json!({"kind":"character"}),
            serde_json::json!({"kind":"tactical","injuries":3}),
        ] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved[class][id.0.to_string()]["crew_recovery"]["mode"] = invalid;
            if let Ok(state) = serde_json::from_value(saved) {
                let mut candidate = world.clone();
                candidate.btech = state;
                assert!(candidate.validate(&config).is_err());
            }
        }
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved[class][id.0.to_string()]["crew_recovery"]["remaining"] = 31.into();
        if let Ok(state) = serde_json::from_value(saved) {
            let mut candidate = world.clone();
            candidate.btech = state;
            assert!(candidate.validate(&config).is_err());
        }
    }
}

/// A powered-down empty vehicle must keep the server heartbeat alive and retry a failed save.
#[tokio::test]
async fn empty_crew_server_recovery_retries_without_spending_unsaved_dice() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, id, _) = fixture(true).await;
        injure_battle_tactical_pilot(&mut world,id,3,false).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        saved["vehicles"][id.0.to_string()]["crew_recovery"]["remaining"] = 1.into();
        world.btech = serde_json::from_value(saved).unwrap();
        assert!(optical_scanner_observers(&world).is_empty());
        let before = recovery(&world,id).clone();
        let mut expected = world.clone();
        advance_battle_units(&mut expected, 0);
        assert_eq!(recovery(&expected,id).remaining,0);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_crew BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'crew recovery failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(recovery(&saved,id), &before);
        sqlx::raw_sql("DROP TRIGGER deny_crew").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let saved = persistence::load(&config.database()).await.unwrap();
                if recovery(&saved,id).remaining == 0 {
                    assert_eq!(recovery(&saved,id), recovery(&expected,id));
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}
