//! Two real cockpit clients fight through the committed server heartbeat and restart.
use crate::support;
use sqlx::Connection;
use std::{cell::Cell, rc::Rc, time::Duration};
use stompymux_rs::{
    BattleCharacter, BattleCharacterValue, BattleDice, BattleMapAsset, BattlePower, BattleTemplate,
    BattleUnit, Config, Kind, ObjectId, ShutdownRequest, World, create_battle_map,
    create_battle_unit, persistence, place_battle_unit, set_battle_character,
    set_battle_character_value,
};

/// Set up ordinary, fully armored opposing units; gameplay starts with both reactors off.
async fn battlefield() -> (tempfile::TempDir, Config, [ObjectId; 2]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Duel field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "duel.map",
        BattleMapAsset::from_cells(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20)))
            .unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for (player, name, template, y) in [
        (
            ObjectId(1),
            "Alpha Atlas",
            include_str!("fixtures/btech/mechs/AS7-D.toml"),
            10,
        ),
        (
            ObjectId(2),
            "Bravo Jenner",
            include_str!("fixtures/btech/mechs/JR7-D.toml"),
            8,
        ),
    ] {
        let id = world.create(&config, name.into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("test", template).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 10, y).unwrap();
        world.objects.get_mut(&player).unwrap().location = Some(map);
        set_battle_character(
            &mut world,
            player,
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
        for skill in [
            "Gunnery-Laser",
            "Gunnery-Missile",
            "Gunnery-Ballistic",
            "Piloting-Biped",
            "Computer",
            "Perception",
        ] {
            set_battle_character_value(
                &mut world,
                player,
                skill,
                BattleCharacterValue {
                    value: 8,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["dice"] = serde_json::to_value(BattleDice::seeded([player.0 as u8; 32])).unwrap();
        // A terminal hit can injure the empty cockpit, whose recovery later transfers to its pilot.
        unit["crew_recovery"]["dice"] =
            serde_json::to_value(BattleDice::seeded([player.0 as u8 + 20; 32])).unwrap();
        unit["signature"]["team"] = player.0.into();
        let heading = if player.0 == 1 { 0.0 } else { 180.0 };
        unit["motion"]["heading"] = heading.into();
        unit["motion"]["desired_heading"] = heading.into();
        state["recoveries"][player.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([player.0 as u8 + 10; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        units.push(id);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE player_state SET password_hash=? WHERE object_dbref IN (1,2)")
        .bind(stompymux_rs::accounts::hash("secret", &config).unwrap())
        .execute(&mut sql)
        .await
        .unwrap();
    (dir, config, units.try_into().unwrap())
}

/// Log in at a scenario location without assuming the shared fixture's default room.
async fn login(address: std::net::SocketAddr, player: i64, room: &str) -> support::Client {
    let mut client = support::Client {
        socket: tokio::net::TcpStream::connect(address).await.unwrap(),
        pending: Vec::new(),
    };
    client.until("Who are you? ").await;
    client.send(&format!("#{player}")).await;
    client.until("Password: ").await;
    client.send("secret").await;
    client.until(room).await;
    client
}

/// A normal speech command fences responses, including rejected combat commands.
async fn command(client: &mut support::Client, sequence: &mut u32, input: &str) -> String {
    *sequence += 1;
    let marker = format!("__duel_response_{}__", *sequence);
    client.send(input).await;
    client.send(&format!("say {marker}")).await;
    client.until(&marker).await
}

/// Advance one ordinary heartbeat and wait for its durable state change.
/// Resume time before socket/SQLite work, avoiding paused-clock timeout auto-advancement.
async fn tick(config: &Config) -> World {
    let before = persistence::load(&config.database()).await.unwrap().btech;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let world = persistence::load(&config.database()).await.unwrap();
            if world.btech != before {
                return world;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("heartbeat must commit before the next simulated second")
}

/// Cockpit assignment can transfer a pending empty-crew recovery to its player.
async fn await_recovery(config: &Config, player: ObjectId) {
    for _ in 0..300 {
        if !persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .unconscious(player)
        {
            return;
        }
        tick(config).await;
    }
    let world = persistence::load(&config.database()).await.unwrap();
    panic!(
        "Pilot {player:?} should recover before testing wreck controls: {:?}",
        world.btech.recoveries().get(&player)
    );
}

#[tokio::test]
async fn two_clients_acquire_lock_fire_destroy_and_restart() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, units) = battlefield().await;
            let (address, shutdown, task, _lua) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            let mut clients = [
                login(address, 1, "Duel field").await,
                login(address, 2, "Duel field").await,
            ];
            let mut sequence = 0;
            for (client, unit) in clients.iter_mut().zip(units) {
                command(client, &mut sequence, &format!("enter #{}", unit.0)).await;
                let output = command(client, &mut sequence, "pilot").await;
                assert!(output.contains("take the cockpit"), "{output}");
                let output = command(client, &mut sequence, "startup").await;
                assert!(output.contains("Startup Cycle commencing"), "{output}");
            }
            let mut world = persistence::load(&config.database()).await.unwrap();
            for _ in 0..40 {
                if units
                    .iter()
                    .all(|id| world.btech.constructed_units()[id].power() == BattlePower::Running)
                {
                    break;
                }
                world = tick(&config).await;
            }
            assert!(
                units
                    .iter()
                    .all(|id| world.btech.constructed_units()[id].power() == BattlePower::Running)
            );
            // Startup completion enables scanning on the following heartbeat.
            for _ in 0..30 {
                if units.iter().all(|id| {
                    !stompymux_rs::visible_battle_contacts(&world, *id)
                        .unwrap()
                        .is_empty()
                }) {
                    break;
                }
                world = tick(&config).await;
            }
            for (side, client) in clients.iter_mut().enumerate() {
                let output = command(client, &mut sequence, "contacts").await;
                assert!(
                    output.contains(&format!(
                        "[{}]",
                        world.btech.constructed_units()[&units[1 - side]]
                            .battlefield_id()
                            .unwrap()
                    )),
                    "{output}"
                );
                let output = command(
                    client,
                    &mut sequence,
                    &format!("lock #{}", units[1 - side].0),
                )
                .await;
                assert!(output.contains("Target set"), "{output}");
            }
            for _ in 0..8 {
                world = tick(&config).await;
            }
            for id in units {
                assert_eq!(
                    world.btech.constructed_units()[&id]
                        .target_lock()
                        .unwrap()
                        .remaining,
                    0
                );
            }
            let initial = world.btech.clone();
            let mut shots = [0; 2];
            let mut transcript = String::new();
            let mut received = [String::new(), String::new()];
            'fight: for _ in 0..240 {
                // The lighter mech gets the first opportunity in each committed second.
                for side in [1, 0] {
                    let shooter = units[side];
                    let target = units[1 - side];
                    let count = world.btech.constructed_units()[&shooter]
                        .loadout()
                        .unwrap()
                        .weapons
                        .len();
                    for index in 0..count {
                        if units
                            .iter()
                            .any(|id| world.btech.constructed_units()[id].is_destroyed())
                        {
                            break 'fight;
                        }
                        let unit = &world.btech.constructed_units()[&shooter];
                        if world.btech.unconscious(ObjectId(side as i64 + 1))
                            || !unit.weapon_readiness(index).unwrap().ready
                            || !stompymux_rs::battle_weapon_bears_on(&world, shooter, target, index)
                                .unwrap()
                        {
                            continue;
                        }
                        let output =
                            command(&mut clients[side], &mut sequence, &format!("fire {index}"))
                                .await;
                        assert!(output.contains("You fire"), "{output}\n{transcript}");
                        transcript.push_str(&output);
                        received[side].push_str(&output);
                        shots[side] += 1;
                        world = persistence::load(&config.database()).await.unwrap();
                    }
                }
                world = tick(&config).await;
            }
            assert!(
                shots.iter().all(|n| *n > 0),
                "both pilots must fire: {shots:?}"
            );
            let loser = units
                .iter()
                .position(|id| world.btech.constructed_units()[id].is_destroyed())
                .unwrap_or_else(|| panic!("duel did not finish: {shots:?}\n{transcript}"));
            for id in units {
                assert_ne!(
                    world.btech.constructed_units()[&id].sections(),
                    initial.constructed_units()[&id].sections()
                );
            }
            let dead = &world.btech.constructed_units()[&units[loser]];
            assert_eq!(dead.power(), BattlePower::Off);
            assert_eq!(dead.pilot(), None);
            assert_eq!(dead.target_lock(), None);
            assert!(dead.weapon_recycle().is_empty());
            assert_eq!(dead.motion().unwrap().speed, 0.0);
            assert_eq!(dead.motion().unwrap().desired_speed, 0.0);
            for (side, client) in clients.iter_mut().enumerate() {
                received[side].push_str(&command(client, &mut sequence, "weapons").await);
                let outcome = if side == loser {
                    "You have been destroyed!"
                } else {
                    "You destroyed the target!"
                };
                assert_eq!(
                    received[side].matches(outcome).count(),
                    1,
                    "{}",
                    received[side]
                );
                let other = if side == loser {
                    "You destroyed the target!"
                } else {
                    "You have been destroyed!"
                };
                assert!(!received[side].contains(other), "{}", received[side]);
                assert!(
                    received[side].contains(" has fired a "),
                    "{}",
                    received[side]
                );
                assert_eq!(
                    world.objects[&ObjectId(side as i64 + 1)].location,
                    Some(units[side])
                );
            }
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
            let saved = persistence::load(&config.database()).await.unwrap();
            let dead = saved.btech.constructed_units()[&units[loser]].clone();
            let (address, shutdown, task, _lua) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            let name = if loser == 0 {
                "Alpha Atlas"
            } else {
                "Bravo Jenner"
            };
            let mut client = login(address, loser as i64 + 1, name).await;
            let fire = format!("fire 0 #{}", units[1 - loser].0);
            let output = command(&mut client, &mut sequence, &fire).await;
            assert!(
                output.contains("Take the cockpit with pilot first"),
                "{output}"
            );
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_wreck_unchanged(&loaded.btech.constructed_units()[&units[loser]], &dead);
            // A surviving pilot can still be recovering from the final hit after restart.
            let player = ObjectId(loser as i64 + 1);
            await_recovery(&config, player).await;
            // An explicit new cockpit claim must not make a wreck operational again.
            let output = command(&mut client, &mut sequence, "pilot").await;
            assert!(output.contains("take the cockpit"), "{output}");
            await_recovery(&config, player).await;
            let claimed = persistence::load(&config.database()).await.unwrap();
            let output = command(&mut client, &mut sequence, "startup").await;
            assert!(output.contains("Destroyed unit cannot start"), "{output}");
            let output = command(&mut client, &mut sequence, &fire).await;
            assert!(output.contains("Unit is destroyed"), "{output}");
            let rejected = persistence::load(&config.database()).await.unwrap();
            assert_wreck_unchanged(
                &rejected.btech.constructed_units()[&units[loser]],
                &claimed.btech.constructed_units()[&units[loser]],
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Rejected controls preserve the wreck while heat and reactor windows continue advancing.
fn assert_wreck_unchanged(after: &BattleUnit, before: &BattleUnit) {
    assert!(after.heat().stored <= before.heat().stored);
    // Excess is sampled on the heartbeat; a post-shot sample may rise even
    // while stored heat cools. It cannot exceed the earlier thermal energy.
    assert!(after.heat().excess <= before.heat().stored.max(before.heat().excess));
    let actual = serde_json::to_value(after).unwrap();
    let mut expected = serde_json::to_value(before).unwrap();
    expected["heat"] = actual["heat"].clone();
    match (
        after.reactor_instability_remaining(),
        before.reactor_instability_remaining(),
    ) {
        (Some(after), Some(before)) => assert!(after <= before),
        (after, before) => assert_eq!(after, before),
    }
    expected["reactor_instability_remaining"] = actual["reactor_instability_remaining"].clone();
    assert!(!after.fired_recently() || before.fired_recently());
    expected["fired_recently"] = actual["fired_recently"].clone();
    assert_eq!(actual, expected);
}
