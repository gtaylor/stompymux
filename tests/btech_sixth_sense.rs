//! Lock warnings share startup sampling, random draws and durable delivery across chassis.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Inspect the serialized authority rather than maintaining a parallel test event queue.
fn state(world: &World, id: ObjectId) -> serde_json::Value {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()].clone()
}

/// Select a deterministic successful roll with each possible delay, or a failed warning roll.
fn dice(delay: Option<u16>) -> (BattleDice, BattleDice) {
    for seed in 0..=255 {
        let initial = BattleDice::seeded([seed; 32]);
        let mut after = initial.clone();
        let success = after.two_d6() <= 8;
        if match delay {
            Some(delay) => success && after.die(3).unwrap() == delay,
            None => !success,
        } {
            return (initial, after);
        }
    }
    panic!("No seed for requested warning outcome");
}

/// Assign a real connected recipient; startup behavior is tested independently below.
fn recipient(config: &Config, world: &mut World, target: ObjectId) -> ObjectId {
    let pilot = world.create(config, "Recipient".into(), Kind::Player);
    let object = world.objects.get_mut(&pilot).unwrap();
    object.location = Some(target);
    object.flags.insert(Flag::Connected);
    assign_battle_pilot(world, target, pilot).unwrap();
    firing::edit(world, target, |s| s["sixth_sense"]["enabled"] = true.into());
    pilot
}

/// Every source/target chassis pair consumes exactly the warning dice and retains all queued locks.
#[tokio::test]
async fn warnings_share_randomness_delay_and_restart_across_chassis() {
    for source in firing::templates() {
        for target_source in firing::templates() {
            let (_dir, config, mut world, source, target, _) =
                firing::fixture_with_target(&source, None, &target_source).await;
            let pilot = recipient(&config, &mut world, target);
            let target_dice = state(&world, target)["dice"].clone();
            for delay in 1..=3 {
                let (initial, after) = dice(Some(delay));
                firing::edit(&mut world, source, |s| {
                    s["dice"] = serde_json::to_value(initial).unwrap()
                });
                select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
                assert_eq!(
                    state(&world, source)["dice"],
                    serde_json::to_value(after).unwrap()
                );
            }
            assert_eq!(
                state(&world, target)["sixth_sense"]["pending"]
                    .as_array()
                    .unwrap()
                    .len(),
                3
            );
            assert_eq!(state(&world, target)["dice"], target_dice);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            // Session connectivity is intentionally not persisted.
            restored
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            for _ in 0..3 {
                let expected = advance_battle_sixth_sense(&mut world);
                assert_eq!(expected.len(), 1);
                assert_eq!(expected[0].0, pilot);
                assert!(!expected[0].1.contains("Sighter"));
                assert_eq!(advance_battle_sixth_sense(&mut restored), expected);
            }
            assert!(advance_battle_sixth_sense(&mut world).is_empty());
            assert!(
                state(&world, target)["sixth_sense"]["pending"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

/// Disabled abilities skip random draws; failed rolls do not draw a delay or enqueue a warning.
#[tokio::test]
async fn warning_rolls_obey_ability_and_success_gates() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, target, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let before = state(&world, source)["dice"].clone();
        select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
        assert_eq!(state(&world, source)["dice"], before);
        recipient(&config, &mut world, target);
        set_battle_observer(&mut world, source, true).unwrap();
        select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
        assert_eq!(state(&world, source)["dice"], before);
        assert!(
            state(&world, target)["sixth_sense"]["pending"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        set_battle_observer(&mut world, source, false).unwrap();
        let (initial, after) = dice(None);
        firing::edit(&mut world, source, |s| {
            s["dice"] = serde_json::to_value(initial).unwrap()
        });
        select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
        assert_eq!(
            state(&world, source)["dice"],
            serde_json::to_value(after).unwrap()
        );
        assert!(
            state(&world, target)["sixth_sense"]["pending"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let before = world.btech.clone();
        select_battle_target(&mut world, source, ObjectId(1), None).unwrap();
        assert_eq!(
            state(&world, source)["dice"],
            serde_json::to_value(&before).unwrap()[if world.btech.vehicles().contains_key(&source) {
                "vehicles"
            } else {
                "constructed"
            }][source.0.to_string()]["dice"]
        );
    }
}

/// A warning due while disconnected is discarded rather than held until the pilot reconnects.
#[tokio::test]
async fn delivery_checks_current_active_pilot() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, target, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let pilot = recipient(&config, &mut world, target);
        let (initial, _) = dice(Some(1));
        firing::edit(&mut world, source, |s| {
            s["dice"] = serde_json::to_value(initial).unwrap()
        });
        select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
        let mut unconscious = world.clone();
        unconscious
            .objects
            .get_mut(&target)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut unconscious,
            pilot,
            BattleCharacter {
                bruise: 0,
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
            .find(|&seed| BattleDice::seeded([seed; 32]).two_d6() == 2)
            .unwrap();
        let mut saved = serde_json::to_value(&unconscious.btech).unwrap();
        saved["recoveries"][pilot.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        unconscious.btech = serde_json::from_value(saved).unwrap();
        injure_battle_character_pilot(&mut unconscious, target, 6, false).unwrap();
        assert!(unconscious.btech.unconscious(pilot));
        assert!(advance_battle_sixth_sense(&mut unconscious).is_empty());
        assert!(
            state(&unconscious, target)["sixth_sense"]["pending"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Connected);
        assert!(advance_battle_sixth_sense(&mut world).is_empty());
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        assert!(advance_battle_sixth_sense(&mut world).is_empty());
    }
}

/// Startup samples the advantage at completion; edits during normal operation leave the cache alone.
#[tokio::test]
async fn startup_captures_the_current_pilots_advantage() {
    for template in firing::templates() {
        let (_dir, _config, mut world, source, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                bruise: 0,
                lethal: 0,
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
            },
        )
        .unwrap();
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Sixth_Sense",
            BattleCharacterValue {
                value: 1,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        assert_eq!(state(&world, source)["sixth_sense"]["enabled"], false);
        stop_battle_unit(
            &mut world,
            source,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
        for _ in 0..4 {
            advance_battle_units(&mut world, 0);
        }
        assert_eq!(state(&world, source)["sixth_sense"]["enabled"], false);
        advance_battle_units(&mut world, 0);
        assert_eq!(state(&world, source)["sixth_sense"]["enabled"], true);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Sixth_Sense",
            BattleCharacterValue {
                value: 0,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        advance_battle_units(&mut world, 0);
        assert_eq!(state(&world, source)["sixth_sense"]["enabled"], true);
        stop_battle_unit(
            &mut world,
            source,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        assert_eq!(state(&world, source)["sixth_sense"]["enabled"], false);
    }
}

/// Native and Lua locks schedule identical warnings; callback abort restores both queue and dice.
#[tokio::test]
async fn lock_transactions_and_private_publication_share_one_path() {
    use std::{cell::RefCell, rc::Rc};
    for template in firing::templates() {
        let (_dir, config, mut world, source, target, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let pilot = recipient(&config, &mut world, target);
        let passenger = world.create(&config, "Passenger".into(), Kind::Player);
        world.objects.get_mut(&passenger).unwrap().location = Some(target);
        let (initial, _) = dice(Some(1));
        firing::edit(&mut world, source, |s| {
            s["dice"] = serde_json::to_value(initial).unwrap()
        });
        let baseline = world.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.lock({},1,{}); error('abort')",
                    source.0, target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, baseline.btech);
        assert!(scripts.drain_outbox().is_empty());
        scripts
            .eval_callback::<()>(&format!("btech.unit.lock({},1,{})", source.0, target.0))
            .unwrap();
        let expected = scripts.world().btech.clone();
        *scripts.world_mut() = baseline;
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("lock #{}", target.0),
        );
        assert!(reply.contains("Target set"), "{reply}");
        assert_eq!(scripts.world().btech, expected);
        scripts.drain_outbox();
        let notices = advance_battle_sixth_sense_action(&scripts).unwrap();
        assert_eq!(
            notices,
            vec![(pilot, "You have a bad feeling about this..".into())]
        );
        let output = scripts.drain_outbox();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].0, pilot);
        assert!(
            advance_battle_sixth_sense_action(&scripts)
                .unwrap()
                .is_empty()
        );
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// A cold battlefield still ticks warnings, and a failed database commit publishes no early warning.
#[tokio::test(flavor = "current_thread")]
async fn server_retries_warning_delivery_after_failed_commit() {
    use sqlx::{Connection, SqliteConnection};
    use std::{cell::Cell, rc::Rc, time::Duration};
    tokio::task::LocalSet::new().run_until(async {
        for template in [include_str!("../game/mechs/JR7-D.toml"), include_str!("../game/mechs/Demolisher.toml")] {
            let (_dir, config, mut world, source, target, _) = firing::fixture_with_target(template, None, template).await;
            for id in [source, target] {
                world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
                assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
                stop_battle_unit(&mut world, id, ObjectId(1), BattleMovementRules::STANDARD.fall).unwrap();
            }
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(source);
            assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
            firing::edit(&mut world, source, |s| s["sixth_sense"] = serde_json::json!({"enabled":true,"pending":[[1,4]]}));
            world.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &config).unwrap());
            persistence::save(&config.database(), &world).await.unwrap();
            let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
            if world.btech.vehicles().contains_key(&source) {
                sqlx::query("CREATE TRIGGER deny_warning BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'warning failure'); END").execute(&mut sql).await.unwrap();
            } else {
                sqlx::query("CREATE TRIGGER deny_warning BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'warning failure'); END").execute(&mut sql).await.unwrap();
            }
            let (addr, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
            let mut client = support::Client { socket:tokio::net::TcpStream::connect(addr).await.unwrap(), pending:Vec::new() };
            client.until("Who are you? ").await;
            client.send("#1").await;
            client.until("Password: ").await;
            client.send("secret").await;
            client.until("Sighter").await;
            tokio::time::sleep(Duration::from_millis(1200)).await;
            let saved = persistence::load(&config.database()).await.unwrap();
            assert_eq!(state(&saved,source)["sixth_sense"]["pending"], serde_json::json!([[1,4]]));
            client.send("look").await;
            let output = client.until("Sighter").await;
            assert!(!output.contains("You have a slightly bad feeling about this.."));
            sqlx::query("DROP TRIGGER deny_warning").execute(&mut sql).await.unwrap();
            client.until("You have a slightly bad feeling about this..").await;
            let saved = persistence::load(&config.database()).await.unwrap();
            assert!(state(&saved,source)["sixth_sense"]["pending"].as_array().unwrap().is_empty());
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        }
    }).await;
}
