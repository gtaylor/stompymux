//! Vehicle feed clearing preserves saved countdowns, skill dice, ammunition and host rollback.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Install deterministic shooter dice while keeping the rest of construction unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        })
        .unwrap();
}

/// Set up one jammed feed without introducing a firing or recycle event.
fn jam(world: &mut World, id: ObjectId) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["jammed_weapons"] = serde_json::json!([0]);
        })
        .unwrap();
}

#[tokio::test]
async fn vehicle_feed_clearing_replays_skill_success_and_failure() {
    for (weapon, target) in [("IS.RotaryAC/2", 9), ("IS.LRM-5", 6)] {
        let template = include_str!("../game/mechs/Demolisher.toml").replace("IS.AC/20", weapon);
        let (_dir, config, mut base, id) = fixture(&template).await;
        jam(&mut base, id);
        for success in [false, true] {
            let value = (0..=255)
                .find(|value| (BattleDice::seeded([*value; 32]).two_d6() >= target) == success)
                .unwrap();
            let mut world = base.clone();
            seed(&mut world, id, value);
            let mut dice = BattleDice::seeded([value; 32]);
            let roll = dice.two_d6();
            let ammo: u16 = world.btech.vehicles()[&id].ammunition().iter().sum();
            begin_battle_unjam(&mut world, id, ObjectId(1), 0).unwrap();
            for _ in 0..59 {
                assert!(
                    advance_battle_unjamming(&mut world, false, false)
                        .unwrap()
                        .is_empty()
                );
            }
            assert_eq!(world.btech.vehicles()[&id].unjam().unwrap().remaining, 1);
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let messages = advance_battle_unjamming(&mut world, false, false).unwrap();
            assert_eq!(
                messages,
                advance_battle_unjamming(&mut replay, false, false).unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            assert!(
                messages
                    .iter()
                    .any(|(_, text)| text.contains(&format!("Roll: {roll}")))
            );
            assert!(world.btech.vehicles()[&id].unjam().is_none());
            assert_eq!(
                world.btech.vehicles()[&id].weapon_jammed(0).unwrap(),
                !success
            );
            assert_eq!(
                world.btech.vehicles()[&id].ammunition().iter().sum::<u16>(),
                ammo - u16::from(success)
            );
            assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_feed_clearing_admission_lua_rollback_and_silent_expiry() {
    let (_dir, config, mut base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let before = base.btech.clone();
    assert!(begin_battle_unjam(&mut base, id, ObjectId(1), 0).is_err());
    assert_eq!(base.btech, before);
    jam(&mut base, id);
    assert!(begin_battle_unjam(&mut base, id, ObjectId(2), 0).is_err());
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.unjam({},1,0); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "unjam 0").contains("begin to shake")
    );
    assert_eq!(
        scripts
            .eval_callback::<u8>(&format!(
                "return btech.unit.state({}).unjam.remaining",
                id.0
            ))
            .unwrap(),
        60
    );
    let before = scripts.world().btech.clone();
    assert!(begin_battle_unjam(&mut scripts.world_mut(), id, ObjectId(1), 0).is_err());
    assert_eq!(scripts.world().btech, before);
    for condition in ["empty", "off", "destroyed"] {
        let mut world = base.clone();
        seed(&mut world, id, 17);
        let mut dice = BattleDice::seeded([17; 32]);
        begin_battle_unjam(&mut world, id, ObjectId(1), 0).unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        if condition == "empty" {
            saved["vehicles"][id.0.to_string()]["ammunition"] = serde_json::json!([0, 0, 0, 0]);
            world.btech = serde_json::from_value(saved.clone()).unwrap();
        } else if condition == "off" {
            stop_battle_unit(
                &mut world,
                id,
                ObjectId(1),
                BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
        } else {
            let location = world.btech.vehicles()[&id].loadout().unwrap().weapons[0].criticals[0];
            destroy_battle_vehicle_critical(&mut world, id, location).unwrap();
        }
        for remaining in [0, 61] {
            let mut bad = saved.clone();
            bad["vehicles"][id.0.to_string()]["unjam"]["remaining"] = serde_json::json!(remaining);
            assert!(serde_json::from_value::<BtechState>(bad).is_err());
        }
        for _ in 0..59 {
            advance_battle_unjamming(&mut world, false, false).unwrap();
        }
        let messages = advance_battle_unjamming(&mut world, false, false).unwrap();
        assert_eq!(messages.is_empty(), condition != "empty");
        assert!(world.btech.vehicles()[&id].unjam().is_none());
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn vehicle_feed_clearing_character_xp_and_output_roll_back_together() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    jam(&mut world, id);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Tracked",
        BattleCharacterValue {
            value: 0,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut world, id, value);
    begin_battle_unjam(&mut world, id, ObjectId(1), 0).unwrap();
    let mut channel = Channel::new("MechPilotXP".into());
    channel.users.push(communication::Membership {
        who: ObjectId(1),
        listening: true,
    });
    world.channels.insert("MechPilotXP".into(), channel);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(advance_battle_unjamming(&mut scripts.world_mut(), true, true).is_err());
    assert_eq!(scripts.world().btech, before);
    for _ in 0..59 {
        advance_battle_unjamming_action(&scripts, &config, true, true).unwrap();
    }
    assert!(scripts.drain_outbox().is_empty());
    let before = scripts.world().clone();
    scripts
        .world_mut()
        .channels
        .get_mut("MechPilotXP")
        .unwrap()
        .messages = i64::MAX;
    assert!(advance_battle_unjamming_action(&scripts, &config, true, true).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = before;
    advance_battle_unjamming_action(&scripts, &config, true, true).unwrap();
    assert!(
        !scripts.world().btech.vehicles()[&id]
            .weapon_jammed(0)
            .unwrap()
    );
    assert!(!scripts.world().channels["MechPilotXP"].history.is_empty());
}

#[tokio::test]
async fn idle_vehicle_feed_countdown_retries_failed_server_commits() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
        jam(&mut world, id);
        begin_battle_unjam(&mut world, id, ObjectId(1), 0).unwrap();
        assert!(battle_contact_observers(&world).is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // A running countdown keeps its timer row still; refuse the commit at the snapshot stamp.
        sqlx::raw_sql("CREATE TRIGGER deny_unjam BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'unjam failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech.vehicles()[&id].unjam().unwrap().remaining, 60);
        sqlx::raw_sql("DROP TRIGGER deny_unjam").execute(&mut sql).await.unwrap();
        heartbeats.until_saved(&config, 5, |saved| saved.btech.vehicles()[&id].unjam().unwrap().remaining < 60).await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn vehicle_feed_clearing_broadcasts_only_to_current_contacts() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    jam(&mut world, id);
    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        observer,
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    world
        .btech
        .rewrite_unit_record(observer, |record| {
            record["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        })
        .unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut world, id, value);
    begin_battle_unjam(&mut world, id, ObjectId(1), 0).unwrap();
    for _ in 0..59 {
        advance_battle_unjamming(&mut world, false, false).unwrap();
    }
    let mut unseen = world.clone();
    unseen
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    let visible = advance_battle_unjamming(&mut world, false, false).unwrap();
    let hidden = advance_battle_unjamming(&mut unseen, false, false).unwrap();
    assert!(visible.iter().any(|(recipient, text)| *recipient
        == BattleMessageTarget::Unit(observer)
        && text.contains("ejects a mangled shell")));
    assert!(
        !hidden
            .iter()
            .any(|(recipient, _)| *recipient == BattleMessageTarget::Unit(observer))
    );
}
