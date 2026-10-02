//! Targeted radio shares admission, identity visibility and publication across unit types.
use crate::support;
use stompymux_rs::*;

/// Update one chassis's saved facts without duplicating the mixed-pair scenarios.
fn fact(world: &mut World, id: ObjectId, update: impl FnOnce(&mut serde_json::Value)) {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut state = serde_json::to_value(&world.btech).unwrap();
    update(&mut state[key][id.0.to_string()]);
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn targeted_radio_mixed_pairs_share_visibility_delivery_and_restart() {
    const CHASSIS: [&str; 3] = [
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ];
    for sender_template in CHASSIS {
        for target_template in CHASSIS {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Radio field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "radio",
                BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
            )
            .unwrap();
            let listener = world.create(&config, "Listener".into(), Kind::Player);
            let mut ids = Vec::new();
            for (template, pilot, x) in [
                (sender_template, ObjectId(1), 0),
                (target_template, listener, 1),
            ] {
                let id = world.create(&config, "Radio unit".into(), Kind::Thing);
                BattleUnitTemplate::parse("test", template)
                    .unwrap()
                    .create(&mut world, id)
                    .unwrap();
                place_battle_unit(&mut world, id, map, x, 0).unwrap();
                let player = world.objects.get_mut(&pilot).unwrap();
                player.location = Some(id);
                player.flags.insert(Flag::Connected);
                assign_battle_pilot(&mut world, id, pilot).unwrap();
                start_battle_unit(&mut world, id, pilot, true).unwrap();
                ids.push(id);
            }
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            let (sender, target) = (ids[0], ids[1]);
            assert!(resolve_targeted_radio(&world, sender, ObjectId(1), target, "unseen").is_err());
            fact(&mut world, sender, |state| {
                state["contacts"][target.0.to_string()] = serde_json::json!({"identified": false})
            });
            let one_way =
                resolve_targeted_radio(&world, sender, ObjectId(1), target, "Hello").unwrap();
            assert_eq!(one_way.notices.len(), 2);
            assert_eq!(
                one_way.notices[1].text,
                "something [AA] radios you with, 'Hello'"
            );
            fact(&mut world, target, |state| {
                state["contacts"][sender.0.to_string()] = serde_json::json!({"identified": false})
            });
            let report =
                resolve_targeted_radio(&world, sender, ObjectId(1), target, "Hello").unwrap();
            assert!(report.notices[1].text.contains("[aa] radios you"));
            assert!(!report.notices[1].text.starts_with("something"));
            world.validate(&config).unwrap();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let text = support::run_text(&native, &config, ObjectId(1), 1, "radio ab=Hello");
            lua.eval_callback::<()>(&format!(
                "btech.unit.radio_target({},1,{},'Hello')",
                sender.0, target.0
            ))
            .unwrap();
            let output = lua.drain_outbox();
            assert!(
                output
                    .iter()
                    .any(|(id, text)| *id == listener && text.source().contains("radios you"))
            );
            assert_eq!(
                text,
                output
                    .iter()
                    .map(|(_, text)| text.source().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.radio_target({},1,{},'abort'); error('abort')",
                    sender.0, target.0
                ))
                .is_err()
            );
            assert!(lua.drain_outbox().is_empty());
            assert_eq!(lua.world().btech, world.btech);
            assert_eq!(native.world().btech, world.btech);
            assert!(
                resolve_targeted_radio(&world, sender, listener, target, "wrong pilot").is_err()
            );
            for message in ["", "bad\nmessage"] {
                assert!(
                    resolve_targeted_radio(&world, sender, ObjectId(1), target, message).is_err()
                );
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                resolve_targeted_radio(&restored, sender, ObjectId(1), target, "Hello").unwrap(),
                report
            );
            fact(&mut world, target, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
            });
            assert_eq!(
                resolve_targeted_radio(&world, sender, ObjectId(1), target, "Hello")
                    .unwrap()
                    .notices
                    .len(),
                1
            );
            fact(&mut world, sender, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
            });
            assert!(resolve_targeted_radio(&world, sender, ObjectId(1), target, "Hello").is_err());
        }
    }
}

#[tokio::test]
async fn channel_radio_rotates_mixed_transmitters_relays_and_receivers() {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    let templates = [
        include_str!("fixtures/btech/mechs/JR7-D.toml").to_string(),
        include_str!("../game/mechs/GOL-1H.toml").to_string(),
        ground.to_string(),
        ground.replace("Tracked", "Wheeled"),
        ground.replace("Tracked", "Hover"),
        ground.replace("Tracked", "None"),
        include_str!("../game/mechs/Kestrel.toml").to_string(),
    ];
    for rotation in 0..templates.len() {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Channel field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "channel",
            BattleMapAsset::parse("6 2\n.0.0.0.0.0.0\n.0.0.0.0.0.0\n").unwrap(),
        )
        .unwrap();
        let mut ids = Vec::new();
        let mut pilots = Vec::new();
        for index in 0..3 {
            let pilot = if index == 0 {
                ObjectId(1)
            } else {
                world.create(&config, format!("Pilot {index}"), Kind::Player)
            };
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    bruise: 0,
                    lethal: 0,
                    build: 3,
                    reflexes: 3,
                    intuition: 3,
                    learn: 3,
                    charisma: 3,
                },
            )
            .unwrap();
            let id = world.create(&config, "Radio unit".into(), Kind::Thing);
            BattleUnitTemplate::parse("test", &templates[(index + rotation) % templates.len()])
                .unwrap()
                .create(&mut world, id)
                .unwrap();
            place_battle_unit(&mut world, id, map, (index * 2) as i64, 0).unwrap();
            let player = world.objects.get_mut(&pilot).unwrap();
            player.location = Some(id);
            player.flags.insert(Flag::Connected);
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            fact(&mut world, id, |state| {
                state["definition"]["attributes"]["radiotype"] = "117".into();
                state["definition"]["attributes"]["radio_range"] = "2".into();
            });
            set_radio_frequency(&mut world, id, pilot, 0, 42).unwrap();
            let caps = unit_radio_capabilities(&world, id).unwrap();
            assert_eq!(caps.channels, 5);
            assert!(caps.info && caps.scan && caps.relay && caps.digital);
            set_radio_mode(
                &mut world,
                id,
                pilot,
                0,
                BattleRadioMode::parse("DEI", caps).unwrap(),
            )
            .unwrap();
            set_radio_title(&mut world, id, pilot, 0, "Mixed radio").unwrap();
            start_battle_unit(&mut world, id, pilot, true).unwrap();
            ids.push(id);
            pilots.push(pilot);
        }
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        for &id in &ids {
            let key = if world.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            let saved = serde_json::to_value(&world.btech).unwrap();
            assert_eq!(saved[key][id.0.to_string()]["radio_skill"], 12);
        }
        let (sender, relay, receiver) = (ids[0], ids[1], ids[2]);
        world.validate(&config).unwrap();
        let report = resolve_digital_radio(&world, sender, 0, "Hello").unwrap();
        let received = report
            .receptions
            .iter()
            .find(|r| r.receiver == receiver)
            .unwrap();
        assert_eq!(received.transmitters, vec![sender, relay]);
        assert!(received.text.contains("R-path:"));
        let mut disabled = world.clone();
        fact(&mut disabled, relay, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        assert!(
            !resolve_digital_radio(&disabled, sender, 0, "Hello")
                .unwrap()
                .receptions
                .iter()
                .any(|r| r.receiver == receiver)
        );
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "listfreqs");
        assert!(output.contains("Mixed radio"), "{output}");
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "listchannels"),
            output
        );
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "listchannels/invalid")
                .contains("listchannels takes no switches")
        );
        assert_eq!(scripts.world().btech, world.btech);
        scripts.eval_callback::<()>(&format!("btech.unit.radio_mode({},1,0,'DEI'); btech.unit.radio_frequency({},1,0,42); btech.unit.radio_title({},1,0,'Mixed radio')", sender.0, sender.0, sender.0)).unwrap();
        scripts.drain_outbox();
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "sendchannel A=Hello");
        assert!(native.contains("Hello"), "{native}");
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.radio_send({},1,0,'abort'); error('abort')",
                    sender.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, world.btech);
        let mut mined = world.clone();
        set_minefield(
            &mut mined,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 0, y: 0 },
                kind: BattleMineKind::Command,
                strength: 0,
                extra: 42,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let mine_scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(mined.clone())),
        )
        .unwrap();
        assert!(
            mine_scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.radio_send({},1,0,'abort'); error('abort')",
                    sender.0
                ))
                .is_err()
        );
        assert_eq!(mine_scripts.world().btech, mined.btech);
        assert!(mine_scripts.drain_outbox().is_empty());
        let _ =
            send_radio_action(&mine_scripts, &config, sender, ObjectId(1), 0, "detonate").unwrap();
        assert!(
            mine_scripts.world().btech.maps()[&map]
                .minefields()
                .is_empty()
        );
        // All receive a broadcast; only the distant receiver spends interference dice.
        set_radio_mode(&mut world, sender, pilots[0], 0, BattleRadioMode::default()).unwrap();
        let analog_scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        assert!(
            analog_scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.radio_send({},1,0,'scrambled then rolled back'); error('abort')",
                    sender.0
                ))
                .is_err()
        );
        assert_eq!(analog_scripts.world().btech, world.btech);
        assert!(analog_scripts.drain_outbox().is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let analog = resolve_analog_radio(&mut world, sender, 0, "A long radio message").unwrap();
        assert_eq!(
            analog,
            resolve_analog_radio(&mut restored, sender, 0, "A long radio message").unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(analog.receptions.len(), 3);
        assert_eq!(analog.interfered_receivers, vec![receiver]);
        world
            .objects
            .get_mut(&receiver)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        let _ = award_radio_experience(&mut world, &analog, 1000000).unwrap();
        let key = if world.btech.vehicles().contains_key(&receiver) {
            "vehicles"
        } else {
            "constructed"
        };
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()[key][receiver.0.to_string()]["radio_experience_remaining"],
            61
        );
        for _ in 0..61 {
            advance_battle_units(&mut world, 0);
        }
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()[key][receiver.0.to_string()]["radio_experience_remaining"],
            0
        );
        // Unmatched traffic drives the same frequency search on each receiving chassis.
        set_radio_frequency(&mut world, receiver, pilots[2], 0, 1000).unwrap();
        let capabilities = unit_radio_capabilities(&world, receiver).unwrap();
        set_radio_mode(
            &mut world,
            receiver,
            pilots[2],
            0,
            BattleRadioMode::parse("S", capabilities).unwrap(),
        )
        .unwrap();
        let mut scanned = false;
        for _ in 0..20 {
            let report = resolve_analog_radio(&mut world, sender, 0, &"x".repeat(80)).unwrap();
            scanned |= report.scans.iter().any(|scan| scan.receiver == receiver);
        }
        assert!(scanned);
        world.validate(&config).unwrap();
    }
}
