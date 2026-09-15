//! Radio hardware limits, saved settings and native/Lua configuration parity.
use stompymux_rs::*;
mod support;

/// A constructed Jenner with its pilot in the cockpit; channel configuration works while shut down.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Radio Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    (dir, config, world, id)
}

#[test]
fn radio_hardware_quality_and_chassis_defaults() {
    for clan in [false, true] {
        for (quality, channels, range) in [
            (0, if clan { 11 } else { 5 }, if clan { 140 } else { 100 }),
            (1, 2, 64),
            (2, 4, 80),
            (3, 5, 100),
            (4, 8, 120),
            (5, 11, 140),
        ] {
            let mut definition =
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            definition
                .attributes
                .insert("radio".into(), quality.to_string());
            if clan {
                definition
                    .attributes
                    .insert("specials".into(), "Clan FlipArms".into());
                definition.heat_sinks = 24;
                for section in definition.sections.values_mut() {
                    section
                        .criticals
                        .retain(|_, part| part.equipment != "HeatSink");
                }
            }
            if clan {
                for slot in 2..6 {
                    definition
                        .sections
                        .get_mut(&BattleSection::LeftTorso)
                        .unwrap()
                        .criticals
                        .insert(
                            slot,
                            CriticalDefinition {
                                equipment: "HeatSink".into(),
                                data: "-".into(),
                                modes: vec![],
                                brand: None,
                            },
                        );
                }
            }
            let unit = BattleUnit::from_template(definition).unwrap();
            assert_eq!(
                unit.radio_capabilities(),
                BattleRadioCapabilities {
                    channels,
                    range,
                    info: false,
                    scan: false,
                    digital: true,
                    relay: clan || quality >= 4
                }
            );
            assert_eq!(unit.radio_channels().len(), channels as usize);
            let mut explicit_default = unit.definition().clone();
            explicit_default
                .attributes
                .insert("radio_range".into(), "0".into());
            assert_eq!(
                BattleUnit::from_template(explicit_default)
                    .unwrap()
                    .radio_capabilities(),
                unit.radio_capabilities()
            );
            assert!(
                unit.radio_channels()
                    .iter()
                    .all(|c| c == &BattleRadioChannel::default())
            );
        }
    }
    let mut definition = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    definition.attributes.insert("radio".into(), "6".into());
    assert!(BattleUnit::from_template(definition).is_err());
}

#[test]
fn radio_modes_enforce_relay_and_retain_color_parser_order() {
    let capable = BattleRadioCapabilities {
        channels: 8,
        range: 120,
        relay: true,
        info: false,
        scan: false,
        digital: true,
    };
    assert!(BattleRadioMode::parse("E", capable).is_err());
    assert!(
        BattleRadioMode::parse(
            "DE",
            BattleRadioCapabilities {
                relay: false,
                ..capable
            }
        )
        .is_err()
    );
    for unsupported in ["DI", "S"] {
        assert!(BattleRadioMode::parse(unsupported, capable).is_err());
    }
    assert_eq!(
        BattleRadioMode::parse("DuErG", capable).unwrap(),
        BattleRadioMode {
            digital: true,
            muted: true,
            relay: true,
            color: Some('G'),
            ..Default::default()
        }
    );
    assert_eq!(
        BattleRadioMode::parse("D?UE", capable).unwrap(),
        BattleRadioMode {
            digital: true,
            ..Default::default()
        }
    );
    assert_eq!(
        BattleRadioMode::parse("", capable).unwrap(),
        BattleRadioMode::default()
    );
}

#[tokio::test]
async fn radio_settings_native_lua_restart_bounds_and_callback_rollback() {
    let (_dir, config, world, id) = fixture().await;
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
    for (command, call) in [
        (
            "setchannelfreq A=999999",
            format!("btech.unit.radio_frequency({},1,0,999999)", id.0),
        ),
        (
            "setchanneltitle a=long radio title here",
            format!(
                "btech.unit.radio_title({},1,0,'long radio title here')",
                id.0
            ),
        ),
        (
            "setchannelmode A=DuG",
            format!("btech.unit.radio_mode({},1,0,'DuG')", id.0),
        ),
    ] {
        let reply = support::run_text(&native, &config, ObjectId(1), 1, command);
        assert!(reply.contains("Channel A"), "{reply}");
        lua.eval_callback::<()>(&call).unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
    }
    let list = support::run_text(&native, &config, ObjectId(1), 1, "listfreqs");
    assert!(
        list.contains("D-MG") && list.contains("999999") && list.contains("long radio titl"),
        "{list}"
    );
    assert_eq!(
        lua.eval_callback::<u32>(&format!(
            "return btech.unit.state({}).radio[1].frequency",
            id.0
        ))
        .unwrap(),
        999999
    );
    let before = lua.world().clone();
    assert!(lua.eval_callback::<()>(&format!("btech.unit.radio_frequency({},1,0,123); btech.unit.radio_title({},1,0,'abort'); error('abort')",id.0,id.0)).is_err());
    assert_eq!(lua.world().btech, before.btech);
    for command in [
        "setchannelfreq E=1",
        "setchannelfreq A=1000000",
        "setchannelfreq A=-1",
        "setchannelmode A=DE",
        "setchannelfreq AA=1",
    ] {
        support::run_text(&native, &config, ObjectId(1), 1, command);
        assert_eq!(native.world().btech, before.btech);
    }
    let mut saved = before;
    set_radio_title(&mut saved, id, ObjectId(1), 0, &"á".repeat(10)).unwrap();
    assert_eq!(
        saved.btech.constructed_units()[&id].radio_channels()[0].title,
        "á".repeat(7)
    );
    assert!(set_radio_frequency(&mut saved, id, ObjectId(2), 0, 1).is_err());
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
    let mut encoded = serde_json::to_value(&saved.btech).unwrap();
    encoded["constructed"][id.0.to_string()]["radio"][15]["frequency"] = 1000000.into();
    saved.btech = serde_json::from_value(encoded).unwrap();
    assert!(saved.validate(&config).is_err());
}

/// A long flat field with duplicate friendly relays and a final receiver beyond two hops.
async fn relay_fixture() -> (tempfile::TempDir, Config, World, ObjectId, Vec<ObjectId>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Radio field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "radio.map",
        BattleMapAsset::parse(&format!("3 300\n{}", ".0.0.0\n".repeat(300))).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for (quality, y) in [(1, 1), (4, 60), (4, 60), (4, 170), (2, 250)] {
        let id = world.create(&config, "Radio unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mut definition =
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        definition
            .attributes
            .insert("radio".into(), quality.to_string());
        for slot in 2..4 {
            definition
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "Ecm".into(),
                        data: "-".into(),
                        modes: vec![],
                        brand: None,
                    },
                );
        }
        create_battle_unit(&mut world, id, definition).unwrap();
        place_battle_unit(&mut world, id, map, 1, y).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        set_radio_frequency(&mut world, id, ObjectId(1), 0, 42).unwrap();
        let mode = BattleRadioMode {
            digital: true,
            relay: quality == 4,
            ..Default::default()
        };
        set_radio_mode(&mut world, id, ObjectId(1), 0, mode).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        radio_fact(&mut world, id, |u| u["pilot"] = serde_json::Value::Null);
        units.push(id);
    }
    (dir, config, world, map, units)
}

/// Modify serialized fixture facts without spending clock time or changing radio configuration.
fn radio_fact(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let mut value = serde_json::to_value(&world.btech).unwrap();
    change(&mut value["constructed"][id.0.to_string()]);
    world.btech = serde_json::from_value(value).unwrap();
}

#[tokio::test]
async fn digital_radio_directed_relays_channel_selection_and_restart() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    let [source, first, alternative, second, receiver] = units.as_slice() else {
        unreachable!()
    };
    radio_fact(&mut world, *source, |u| {
        u["radio"][0]["title"] = "Command".into()
    });
    radio_fact(&mut world, *receiver, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        u["radio"][0]["mode"]["muted"] = true.into();
        u["radio"][1]["frequency"] = 42.into();
        u["radio"][1]["mode"]["color"] = "G".into();
    });
    let before = world.btech.clone();
    let report = resolve_digital_radio(&world, *source, 0, "Hello").unwrap();
    assert_eq!(world.btech, before);
    assert_eq!(
        report
            .receptions
            .iter()
            .map(|r| r.receiver)
            .collect::<Vec<_>>(),
        units
    );
    let received = report.receptions.last().unwrap();
    assert_eq!(received.transmitters, vec![*source, *first, *second]);
    assert_eq!(received.channel, 1);
    assert_eq!(received.bearing, 0);
    assert_eq!(
        received.text,
        "[fg=green bold][B:000] <Command> Hello[reset]"
    );
    assert_eq!(report.notices().last().unwrap().unit, *receiver);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_digital_radio(&restored, *source, 0, "Hello").unwrap(),
        report
    );
    // A muted relay still carries traffic; disabling its relay flag chooses the equal-length alternative.
    radio_fact(&mut world, *first, |u| {
        u["radio"][0]["mode"]["muted"] = true.into()
    });
    assert_eq!(
        resolve_digital_radio(&world, *source, 0, "Hello")
            .unwrap()
            .receptions
            .last()
            .unwrap()
            .transmitters,
        received.transmitters
    );
    radio_fact(&mut world, *first, |u| {
        u["radio"][0]["mode"]["relay"] = false.into()
    });
    assert_eq!(
        resolve_digital_radio(&world, *source, 0, "Hello")
            .unwrap()
            .receptions
            .last()
            .unwrap()
            .transmitters,
        vec![*source, *alternative, *second]
    );
}

#[tokio::test]
async fn digital_radio_relay_lifecycle_team_and_direction() {
    let (_dir, _config, world, _map, units) = relay_fixture().await;
    let source = units[0];
    let receiver = units[4];
    // Sender range limits each edge: the long-range relay hears the short-range sender only within 64.
    let mut boundary = world.clone();
    relocate_radio_unit(
        &mut boundary,
        units[1],
        world.btech.constructed_units()[&source]
            .position()
            .unwrap()
            .map,
        1,
        65,
    )
    .unwrap();
    assert!(
        resolve_digital_radio(&boundary, source, 0, "edge")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == units[1])
    );
    relocate_radio_unit(
        &mut boundary,
        units[1],
        world.btech.constructed_units()[&source]
            .position()
            .unwrap()
            .map,
        1,
        66,
    )
    .unwrap();
    radio_fact(&mut boundary, units[2], |u| {
        u["radio"][0]["mode"]["relay"] = false.into()
    });
    assert!(
        !resolve_digital_radio(&boundary, source, 0, "edge")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == receiver)
    );
    for mode in ["off", "enemy", "going", "untuned"] {
        let mut changed = world.clone();
        match mode {
            "off" => radio_fact(&mut changed, units[3], |u| {
                u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
            }),
            "enemy" => radio_fact(&mut changed, units[3], |u| {
                u["sensor_signature"]["team"] = 17.into()
            }),
            "going" => {
                changed
                    .objects
                    .get_mut(&units[3])
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
            _ => radio_fact(&mut changed, units[3], |u| {
                u["radio"][0]["frequency"] = 99.into()
            }),
        }
        assert!(
            !resolve_digital_radio(&changed, source, 0, "test")
                .unwrap()
                .receptions
                .iter()
                .any(|r| r.receiver == receiver),
            "{mode}"
        );
    }
    // An enemy can receive directly, but cannot forward the source team's transmission.
    let mut enemy = world.clone();
    radio_fact(&mut enemy, units[1], |u| {
        u["sensor_signature"]["team"] = 17.into()
    });
    assert!(
        resolve_digital_radio(&enemy, source, 0, "test")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == units[1])
    );
}

#[tokio::test]
async fn digital_radio_endpoint_ecm_self_monitor_and_invalid_input() {
    let (_dir, _config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    let jammer = units[2];
    // A hostile nearby emission blocks the source, while preserving its self-monitor reception.
    radio_fact(&mut world, jammer, |u| {
        u["sensor_signature"]["team"] = 17.into();
        u["electronics"]["guardian"] = "ecm".into();
    });
    relocate_radio_unit(&mut world, jammer, map, 1, 2).unwrap();
    let report = resolve_digital_radio(&world, source, 0, "test").unwrap();
    assert_eq!(
        report
            .receptions
            .iter()
            .map(|r| r.receiver)
            .collect::<Vec<_>>(),
        vec![source]
    );
    // Jamming the receiver blocks only that endpoint; a jammed intermediate still forwards.
    relocate_radio_unit(&mut world, jammer, map, 1, 249).unwrap();
    assert!(
        !resolve_digital_radio(&world, source, 0, "test")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == units[4])
    );
    relocate_radio_unit(&mut world, jammer, map, 1, 169).unwrap();
    assert!(
        resolve_digital_radio(&world, source, 0, "test")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == units[4])
    );
    let before = world.btech.clone();
    for message in ["bad\nline", "bad\x1bmarkup", "bad\0null"] {
        assert!(resolve_digital_radio(&world, source, 0, message).is_err());
    }
    assert!(resolve_digital_radio(&world, source, 2, "test").is_err());
    assert!(resolve_digital_radio(&world, source, 1, "test").is_err());
    assert!(resolve_digital_radio(&world, ObjectId(-1), 0, "test").is_err());
    assert_eq!(world.btech, before);
}

/// Move an idle fixture radio while retaining its powered operating state.
fn relocate_radio_unit(
    world: &mut World,
    id: ObjectId,
    map: ObjectId,
    x: i64,
    y: i64,
) -> anyhow::Result<()> {
    radio_fact(world, id, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
    });
    place_battle_unit(world, id, map, x, y)?;
    radio_fact(world, id, |u| {
        u["power"] = serde_json::to_value(BattlePower::Running).unwrap()
    });
    Ok(())
}

#[tokio::test]
async fn analog_radio_range_reception_dice_and_restart_are_atomic() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    let source = units[0];
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = false.into();
        u["radio"][0]["title"] = "Command".into();
    });
    for &id in &units {
        radio_fact(&mut world, id, |u| {
            u["dice"] = serde_json::to_value(BattleDice::seeded([id.0 as u8; 32])).unwrap()
        });
    }
    let before = world.clone();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let report = resolve_analog_radio(&mut world, source, 0, "Message 123").unwrap();
    assert_eq!(
        report
            .receptions
            .iter()
            .map(|r| r.receiver)
            .collect::<Vec<_>>(),
        units
    );
    assert_eq!(report.interfered_receivers, units[3..]);
    assert_eq!(
        report.receptions[1].text,
        "(A:000) <Command> Message 123[reset]"
    );
    assert_eq!(report.notices().len(), 5);
    assert_eq!(
        resolve_analog_radio(&mut restored, source, 0, "Message 123").unwrap(),
        report
    );
    assert_eq!(restored.btech, world.btech);
    for &id in &units {
        let a = serde_json::to_value(&before.btech.constructed_units()[&id]).unwrap();
        let b = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        assert_eq!(
            a["dice"] != b["dice"],
            report.interfered_receivers.contains(&id)
        );
    }
    // A late state rejection rolls back dice already consumed by earlier receivers.
    let mut invalid = before.clone();
    radio_fact(&mut invalid, units[4], |u| {
        u["radio"][15]["frequency"] = 1000000.into()
    });
    let snapshot = invalid.btech.clone();
    assert!(resolve_analog_radio(&mut invalid, source, 0, "Message 123").is_err());
    assert_eq!(invalid.btech, snapshot);
    // Near-range reception, including self-monitoring, consumes no dice at all.
    let mut nearby = before;
    for &id in &units[3..] {
        radio_fact(&mut nearby, id, |u| {
            u["radio"][0]["mode"]["muted"] = true.into()
        });
    }
    let snapshot = nearby.btech.clone();
    assert!(
        resolve_analog_radio(&mut nearby, source, 0, "clear")
            .unwrap()
            .interfered_receivers
            .is_empty()
    );
    assert_eq!(nearby.btech, snapshot);
}

#[tokio::test]
async fn analog_radio_ecm_muting_zero_frequency_and_saved_skill() {
    let (_dir, _config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    let jammer = units[2];
    assert_eq!(world.btech.constructed_units()[&source].radio_skill(), 18);
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = false.into()
    });
    radio_fact(&mut world, jammer, |u| {
        u["sensor_signature"]["team"] = 17.into();
        u["electronics"]["guardian"] = "ecm".into();
        u["radio"][0]["mode"]["muted"] = true.into();
    });
    relocate_radio_unit(&mut world, jammer, map, 1, 2).unwrap();
    let report = resolve_analog_radio(&mut world, source, 0, "under ECM").unwrap();
    assert!(!report.interfered_receivers.contains(&source));
    assert!(report.interfered_receivers.contains(&units[1]));
    assert!(!report.receptions.iter().any(|r| r.receiver == jammer));
    assert_eq!(report.receptions[0].text, "(A:000) under ECM[reset]");
    // Digital selection on the receiving channel does not prevent analog reception.
    assert!(
        world.btech.constructed_units()[&units[1]].radio_channels()[0]
            .mode
            .digital
    );
    assert!(report.receptions.iter().any(|r| r.receiver == units[1]));
    // An unconfigured channel is a real zero-frequency channel, not a disabled radio.
    let zero = resolve_analog_radio(&mut world, source, 1, "zero").unwrap();
    assert_eq!(zero.frequency, 0);
    assert_eq!(zero.receptions.len(), units.len());
    assert!(zero.receptions.iter().all(|r| r.channel == 1));
    let snapshot = world.btech.clone();
    assert!(resolve_analog_radio(&mut world, source, 2, "invalid").is_err());
    assert!(resolve_analog_radio(&mut world, source, 0, "bad\nline").is_err());
    assert!(resolve_analog_radio(&mut world, ObjectId(-1), 0, "missing").is_err());
    assert_eq!(world.btech, snapshot);
    // Invalid mode selection is rejected before any listener dice are spent.
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = true.into()
    });
    let snapshot = world.btech.clone();
    assert!(resolve_analog_radio(&mut world, source, 0, "digital").is_err());
    assert_eq!(world.btech, snapshot);
}

#[tokio::test]
async fn radio_communication_skill_is_captured_only_on_startup_completion() {
    let (_dir, config, mut world, unit) = fixture().await;
    let map = world.create(&config, "Skill field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "skill.map",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, unit, map, 1, 1).unwrap();
    assert_eq!(world.btech.constructed_units()[&unit].radio_skill(), 6);
    set_battle_character(
        &mut world,
        ObjectId(1),
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
    start_battle_unit(&mut world, unit, ObjectId(1), true).unwrap();
    for _ in 0..4 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(world.btech.constructed_units()[&unit].radio_skill(), 6);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Comm-Conventional",
        BattleCharacterValue {
            value: 4,
            ..Default::default()
        },
    )
    .unwrap();
    advance_battle_units(&mut world, 0);
    assert_eq!(world.btech.constructed_units()[&unit].radio_skill(), 8);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Comm-Conventional",
        BattleCharacterValue {
            value: 5,
            ..Default::default()
        },
    )
    .unwrap();
    advance_battle_units(&mut world, 0);
    assert_eq!(world.btech.constructed_units()[&unit].radio_skill(), 8);
    radio_fact(&mut world, unit, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
    });
    start_battle_unit(&mut world, unit, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(world.btech.constructed_units()[&unit].radio_skill(), 7);
}

/// Assign the existing fixture player to the source cockpit for actual transmissions.
fn radio_sender(world: &mut World, source: ObjectId) {
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(source);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(world, source, ObjectId(1)).unwrap();
}

#[tokio::test]
async fn sendchannel_native_lua_modes_and_frequency_mines_share_one_action() {
    let (_dir, config, base, map, units) = relay_fixture().await;
    let source = units[0];
    for (digital, strength) in [false, true]
        .into_iter()
        .flat_map(|digital| [-2, -1, 0].map(|strength| (digital, strength)))
    {
        let mut world = base.clone();
        radio_sender(&mut world, source);
        radio_fact(&mut world, source, |u| {
            u["radio"][0]["mode"]["digital"] = digital.into();
            u["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        });
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 250 },
                kind: BattleMineKind::Command,
                strength,
                extra: 42,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.radio_send({},1,0,'abort'); error('abort')",
                source.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let reply = support::run_text(&native, &config, ObjectId(1), 1, "sendchannel A=Hello");
        assert!(!reply.contains("Unknown"), "{reply}");
        let mode: String = lua
            .eval_callback(&format!(
                "return btech.unit.radio_send({},1,0,'Hello').delivery.mode",
                source.0
            ))
            .unwrap();
        assert_eq!(mode, if digital { "digital" } else { "analog" });
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(native.world().btech.maps()[&map].minefields().is_empty());
        let output = lua.drain_outbox();
        assert!(
            output
                .iter()
                .any(|(_, text)| text.source().contains("Hello"))
        );
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn transmission_failure_restores_delivery_dice_mines_and_outbox() {
    let (_dir, config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    radio_sender(&mut world, source);
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = false.into()
    });
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 60 },
        kind: BattleMineKind::Command,
        strength: 0,
        extra: 42,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 250 },
            strength: 2,
            ..mine
        }),
    )
    .unwrap();
    support::fail_mine_ignition(&mut world, map, 250);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(send_radio_action(&scripts, &config, source, ObjectId(1), 0, "abort").is_err());
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    // A surrounding Lua callback can also undo an otherwise successful complete transmission.
    set_minefield(&mut world, map, 1, None).unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.radio_send({},1,0,'callback'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    for (pilot, channel, text) in [
        (ObjectId(2), 0, "wrong pilot"),
        (ObjectId(1), 2, "bad channel"),
        (ObjectId(1), 0, ""),
        (ObjectId(1), 0, "bad\nline"),
    ] {
        assert!(send_radio_action(&scripts, &config, source, pilot, channel, text).is_err());
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
    }
    radio_fact(&mut world, source, |u| u["stun_remaining"] = 1.into());
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(send_radio_action(&scripts, &config, source, ObjectId(1), 0, "stunned").is_err());
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
}

/// Existing administrator channels receive radio diagnostics through normal channel membership.
fn radio_audit_channels(world: &mut World) {
    for name in ["MechFreqs", "ZeroFrequencies"] {
        let mut channel = Channel::new(name.into());
        channel.users.push(stompymux_rs::communication::Membership {
            who: ObjectId(1),
            listening: true,
        });
        world.channels.insert(name.into(), channel);
    }
}

#[tokio::test]
async fn radio_frequency_audits_match_each_enemy_channel_with_native_lua_parity() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    let source = units[0];
    radio_sender(&mut world, source);
    radio_audit_channels(&mut world);
    radio_fact(&mut world, units[1], |u| {
        u["sensor_signature"]["team"] = 7.into();
        u["radio"][1]["frequency"] = 42.into();
        u["radio"][1]["mode"]["muted"] = true.into();
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap();
    });
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
    let reply = support::run_text(&native, &config, ObjectId(1), 1, "setchannelfreq A=42");
    assert!(reply.contains("ALERT: Possible abuse"), "{reply}");
    lua.eval_callback::<()>(&format!("btech.unit.radio_frequency({},1,0,42)", source.0))
        .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    for scripts in [&native, &lua] {
        let state = scripts.world();
        let channel = &state.channels["MechFreqs"];
        assert_eq!(channel.messages, 2);
        assert_eq!(channel.history.len(), 2);
        assert!(channel.history.iter().all(|m| {
            m.message
                .contains(&format!("matching #{} (Team 7)!", units[1].0))
        }));
    }
    // Setting the same frequency again still audits; zero never audits matching unset channels.
    assert_eq!(
        set_radio_frequency_action(&lua, &config, source, ObjectId(1), 0, 42)
            .unwrap()
            .len(),
        2
    );
    assert!(
        set_radio_frequency_action(&lua, &config, source, ObjectId(1), 0, 0)
            .unwrap()
            .is_empty()
    );
    assert_eq!(lua.world().channels["MechFreqs"].messages, 4);
    // A later channel overflow restores the setting and the earlier publication in that action.
    world.channels.get_mut("MechFreqs").unwrap().messages = i64::MAX - 1;
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["frequency"] = 9.into()
    });
    let failing = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(set_radio_frequency_action(&failing, &config, source, ObjectId(1), 0, 42).is_err());
    assert_eq!(failing.world().btech, world.btech);
    assert_eq!(
        serde_json::to_value(&failing.world().channels).unwrap(),
        serde_json::to_value(&world.channels).unwrap()
    );
    assert!(failing.drain_outbox().is_empty());
}

#[tokio::test]
async fn zero_frequency_audits_follow_map_character_flag_and_rollback_with_mines() {
    let (_dir, config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    radio_sender(&mut world, source);
    radio_audit_channels(&mut world);
    world
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = send_radio_action(&scripts, &config, source, ObjectId(1), 1, "zero text").unwrap();
    assert_eq!(report.audit_messages.len(), 1);
    assert_eq!(
        report.audit_messages[0].channel,
        BattleChannel::ZeroFrequencies
    );
    assert_eq!(
        report.audit_messages[0].text,
        format!(
            "Player #1 (GOD) in mech #{} (channel B) on map #{} 0-freqs \"zero text\"",
            source.0, map.0
        )
    );
    assert_eq!(scripts.world().channels["ZeroFrequencies"].messages, 1);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .channels["ZeroFrequencies"]
            .messages,
        1
    );
    let before = scripts.world().clone();
    scripts.drain_outbox();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.radio_send({},1,1,'aborted'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(&scripts.world().channels).unwrap(),
        serde_json::to_value(&before.channels).unwrap()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.drain_outbox().is_empty());
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 250 },
            kind: BattleMineKind::Command,
            strength: 2,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    support::fail_mine_ignition(&mut world, map, 250);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(send_radio_action(&scripts, &config, source, ObjectId(1), 1, "mine failure").is_err());
    assert_eq!(
        serde_json::to_value(&scripts.world().channels).unwrap(),
        serde_json::to_value(&world.channels).unwrap()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    set_minefield(&mut world, map, 0, None).unwrap();
    world
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        send_radio_action(&scripts, &config, source, ObjectId(1), 1, "OOC map")
            .unwrap()
            .audit_messages
            .is_empty()
    );
    assert_eq!(scripts.world().channels["ZeroFrequencies"].messages, 0);
}

/// A distant connected character receives analog traffic; source control belongs to the other player.
fn radio_xp_crew(world: &mut World, units: &[ObjectId]) {
    let receiver = units[4];
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(receiver);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(world, receiver, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&receiver)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        world,
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
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(units[0]);
    assign_battle_pilot(world, units[0], ObjectId(2)).unwrap();
    radio_fact(world, units[0], |u| {
        u["radio"][0]["mode"]["digital"] = false.into()
    });
}

#[tokio::test]
async fn radio_xp_strict_interval_restart_and_shutdown_countdown() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    radio_xp_crew(&mut world, &units);
    let receiver = units[4];
    radio_fact(&mut world, receiver, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
    });
    let report = resolve_analog_radio(&mut world, units[0], 0, "distant").unwrap();
    let messages = award_radio_experience(&mut world, &report, 1000).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].text,
        format!("GOD gained 1 Comm-Conventional XP (in #{})", receiver.0)
    );
    assert_eq!(
        world.btech.character_values()[&ObjectId(1)]["Comm-Conventional"].experience_balance(),
        1
    );
    assert_eq!(
        world.btech.constructed_units()[&receiver].radio_experience_remaining(),
        61
    );
    assert!(
        award_radio_experience(&mut world, &report, 1001)
            .unwrap()
            .is_empty()
    );
    for _ in 0..60 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(
        world.btech.constructed_units()[&receiver].radio_experience_remaining(),
        1
    );
    assert!(
        award_radio_experience(&mut world, &report, 1060)
            .unwrap()
            .is_empty()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    restored
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    advance_battle_units(&mut restored, 0);
    assert_eq!(
        award_radio_experience(&mut restored, &report, 1061)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        restored.btech.character_values()[&ObjectId(1)]["Comm-Conventional"].experience_balance(),
        2
    );
    // The ordinary skill interval remains independent of the simulation countdown.
    for _ in 0..61 {
        advance_battle_units(&mut restored, 0);
    }
    assert!(
        award_radio_experience(&mut restored, &report, 1062)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        restored.btech.constructed_units()[&receiver].radio_experience_remaining(),
        61
    );
    radio_fact(&mut restored, receiver, |u| {
        u["radio_experience_remaining"] = 62.into()
    });
    assert!(restored.validate(&config).is_err());
}

#[tokio::test]
async fn radio_xp_ineligible_reception_consumes_gate_and_publication_rolls_back() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    radio_xp_crew(&mut world, &units);
    let receiver = units[4];
    let report = resolve_analog_radio(&mut world, units[0], 0, "test").unwrap();
    let mut disconnected = world.clone();
    disconnected
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Connected);
    assert!(
        award_radio_experience(&mut disconnected, &report, 1000)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        disconnected.btech.constructed_units()[&receiver].radio_experience_remaining(),
        61
    );
    assert!(
        !disconnected
            .btech
            .character_values()
            .get(&ObjectId(1))
            .is_some_and(|v| v.contains_key("Comm-Conventional"))
    );
    let mut ooc = world.clone();
    ooc.objects
        .get_mut(&receiver)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    assert!(
        award_radio_experience(&mut ooc, &report, 1000)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ooc.btech.constructed_units()[&receiver].radio_experience_remaining(),
        0
    );
    let mut channel = Channel::new("MechXP".into());
    channel.messages = i64::MAX;
    world.channels.insert("MechXP".into(), channel);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(send_radio_action(&scripts, &config, units[0], ObjectId(2), 0, "XP failure").is_err());
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
    world.channels.get_mut("MechXP").unwrap().messages = 0;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let transmission =
        send_radio_action(&scripts, &config, units[0], ObjectId(2), 0, "XP success").unwrap();
    assert_eq!(transmission.experience_messages.len(), 1);
    assert_eq!(scripts.world().channels["MechXP"].messages, 1);
    assert_eq!(
        scripts.world().btech.constructed_units()[&receiver].radio_experience_remaining(),
        61
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.radio_send({},2,0,'abort'); error('abort')",
                units[0].0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert_eq!(scripts.world().channels["MechXP"].messages, 0);
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn observer_radio_bypasses_tuning_range_and_ecm_with_identified_clear_text() {
    let (_dir, config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    let observer = units[4];
    let affiliation = world.create(&config, "Faction".into(), Kind::Thing);
    world.objects.get_mut(&source).unwrap().affiliation = Some(affiliation);
    radio_fact(&mut world, source, |u| {
        u["sensor_signature"]["team"] = 17.into()
    });
    set_battle_observer(&mut world, observer, true).unwrap();
    radio_fact(&mut world, observer, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        u["radio"][0]["mode"]["muted"] = true.into();
        u["radio"][1]["mode"]["color"] = "R".into();
    });
    radio_fact(&mut world, units[2], |u| {
        u["sensor_signature"]["team"] = 18.into();
        u["electronics"]["guardian"] = "ecm".into();
    });
    relocate_radio_unit(&mut world, units[2], map, 1, 2).unwrap();
    let digital = resolve_digital_radio(&world, source, 0, "Clear message").unwrap();
    let received = digital
        .receptions
        .iter()
        .find(|r| r.receiver == observer)
        .unwrap();
    assert_eq!(received.channel, 1);
    assert_eq!(received.transmitters, vec![source]);
    assert_eq!(
        received.text,
        "[fg=cyan][B:0] <Faction:AA:42> <> Clear message[reset]"
    );
    assert_eq!(
        digital
            .receptions
            .iter()
            .filter(|r| r.receiver == observer)
            .count(),
        1
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_digital_radio(&restored, source, 0, "Clear message").unwrap(),
        digital
    );
    for candidate in [&mut world, &mut restored] {
        radio_fact(candidate, source, |u| {
            u["radio"][0]["mode"]["digital"] = false.into()
        });
    }
    let analog = resolve_analog_radio(&mut world, source, 0, "Clear message").unwrap();
    assert_eq!(
        resolve_analog_radio(&mut restored, source, 0, "Clear message").unwrap(),
        analog
    );
    assert_eq!(world.btech, restored.btech);
    assert!(analog.interfered_receivers.contains(&observer));
    let received = analog
        .receptions
        .iter()
        .find(|r| r.receiver == observer)
        .unwrap();
    assert_eq!(
        received.text,
        "[fg=cyan](B:0) <Faction:AA:42> <> Clear message[reset]"
    );
    // Dead affiliations disappear from the header; fully muted observers receive nothing.
    world
        .objects
        .get_mut(&affiliation)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let analog = resolve_analog_radio(&mut world, source, 0, "empty affiliation").unwrap();
    assert!(
        analog
            .receptions
            .iter()
            .find(|r| r.receiver == observer)
            .unwrap()
            .text
            .contains("<:AA:42>")
    );
    radio_fact(&mut world, observer, |u| {
        for slot in 0..16 {
            u["radio"][slot]["mode"]["muted"] = true.into();
        }
    });
    assert!(
        !resolve_analog_radio(&mut world, source, 0, "muted")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == observer)
    );
}

#[tokio::test]
async fn observer_role_is_saved_and_battlefield_labels_follow_saved_slots() {
    let (_dir, config, mut world, id) = fixture().await;
    assert!(!world.btech.constructed_units()[&id].is_observer());
    assert_eq!(world.btech.constructed_units()[&id].battlefield_id(), None);
    set_battle_observer(&mut world, id, true).unwrap();
    assert!(world.btech.constructed_units()[&id].is_observer());
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<bool>(&format!("return btech.unit.state({}).observer", id.0))
            .unwrap()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .constructed_units()[&id]
            .is_observer()
    );
    for (slot, label) in [
        (0, "AA"),
        (1, "AB"),
        (25, "AZ"),
        (26, "A0"),
        (35, "A9"),
        (36, "BA"),
        (1295, "99"),
        (1296, "BAA"),
    ] {
        radio_fact(&mut world, id, |u| u["map_slot"] = slot.into());
        assert_eq!(
            world.btech.constructed_units()[&id]
                .battlefield_id()
                .as_deref(),
            Some(label)
        );
    }
    assert!(set_battle_observer(&mut world, ObjectId(1), true).is_err());
    assert!(set_battle_observer(&mut world, ObjectId(-1), true).is_err());
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(set_battle_observer(&mut world, id, false).is_err());
}

/// Acquire a fixture contact without spending scanner dice or changing radio settings.
fn radio_contact(world: &mut World, observer: ObjectId, target: ObjectId) {
    radio_fact(world, observer, |u| {
        u["contacts"][target.0.to_string()] =
            serde_json::json!({"primary": true, "secondary": false})
    });
}

#[tokio::test]
async fn targeted_radio_identity_visibility_native_lua_and_callback_rollback() {
    let (_dir, config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    let target = units[1];
    radio_sender(&mut world, source);
    relocate_radio_unit(&mut world, target, map, 1, 2).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    radio_contact(&mut world, source, target);
    let before = world.btech.clone();
    let report = resolve_targeted_radio(&world, source, ObjectId(1), target, "Hello").unwrap();
    assert_eq!(
        report.notices[0].text,
        "You radio Jenner [ab] with, 'Hello'"
    );
    assert_eq!(
        report.notices[1].text,
        "something [AA] radios you with, 'Hello'"
    );
    assert_eq!(world.btech, before);
    radio_contact(&mut world, target, source);
    let report = resolve_targeted_radio(&world, source, ObjectId(1), target, "Hello").unwrap();
    assert_eq!(
        report.notices[1].text,
        "Jenner [aa] radios you with, 'Hello'"
    );
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
    let native_text = support::run_text(&native, &config, ObjectId(1), 1, "radio ab=Hello");
    lua.eval_callback::<()>(&format!(
        "btech.unit.radio_target({},1,{},'Hello')",
        source.0, target.0
    ))
    .unwrap();
    let lua_text = lua
        .drain_outbox()
        .iter()
        .map(|(_, text)| text.source().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(native_text, lua_text);
    assert!(native_text.contains("radios you"));
    assert_eq!(native.world().btech, world.btech);
    assert_eq!(lua.world().btech, world.btech);
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.radio_target({},1,{},'abort'); error('abort')",
            source.0, target.0
        ))
        .is_err()
    );
    assert!(lua.drain_outbox().is_empty());
    assert_eq!(lua.world().btech, world.btech);
    let by_dbref = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("radio #{}=Hello", target.0),
    );
    assert_eq!(by_dbref, native_text);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_targeted_radio(&restored, source, ObjectId(1), target, "Hello").unwrap(),
        report
    );
}

#[tokio::test]
async fn targeted_radio_power_observer_contact_and_message_guards() {
    let (_dir, _config, mut world, map, units) = relay_fixture().await;
    let source = units[0];
    let target = units[1];
    radio_sender(&mut world, source);
    relocate_radio_unit(&mut world, target, map, 1, 2).unwrap();
    assert!(resolve_targeted_radio(&world, source, ObjectId(1), target, "unseen").is_err());
    radio_contact(&mut world, source, target);
    radio_fact(&mut world, target, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
    });
    let report =
        resolve_targeted_radio(&world, source, ObjectId(1), target, "shutdown target").unwrap();
    assert_eq!(report.notices.len(), 1);
    let before = world.btech.clone();
    for text in ["", "bad\nmessage", "bad\0message"] {
        assert!(resolve_targeted_radio(&world, source, ObjectId(1), target, text).is_err());
    }
    assert!(resolve_targeted_radio(&world, source, ObjectId(2), target, "wrong pilot").is_err());
    assert!(resolve_targeted_radio(&world, source, ObjectId(1), ObjectId(-1), "missing").is_err());
    assert_eq!(world.btech, before);
    set_battle_observer(&mut world, source, true).unwrap();
    assert!(resolve_targeted_radio(&world, source, ObjectId(1), target, "observer").is_err());
    set_battle_observer(&mut world, source, false).unwrap();
    radio_fact(&mut world, source, |u| {
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap()
    });
    assert!(
        resolve_targeted_radio(&world, source, ObjectId(1), target, "shutdown source").is_err()
    );
}

#[tokio::test]
async fn observer_scans_keep_acquisition_and_lock_loss_without_routine_chatter() {
    let (_dir, config, mut normal, map, units) = relay_fixture().await;
    let observer = units[0];
    let target = units[1];
    radio_sender(&mut normal, observer);
    relocate_radio_unit(&mut normal, target, map, 1, 2).unwrap();
    set_battle_autocon_shutdown(&mut normal, observer, ObjectId(1), true).unwrap();
    let mut observing = normal.clone();
    set_battle_observer(&mut observing, observer, true).unwrap();
    let normal_events = refresh_optical_scanners(&mut normal, &[observer]).unwrap();
    let events = refresh_optical_scanners(&mut observing, &[observer]).unwrap();
    assert_eq!(events, normal_events);
    assert!(events.iter().any(|e| e.target == target && e.acquired));
    assert!(events.iter().all(|e| e.notice(&observing).is_none()));
    assert!(normal_events.iter().all(|e| e.notice(&normal).is_some()));
    let mut compared = observing.clone();
    set_battle_observer(&mut compared, observer, false).unwrap();
    assert_eq!(compared.btech, normal.btech);
    let _ = select_battle_target(&mut observing, observer, ObjectId(1), Some(target)).unwrap();
    let _ = select_battle_target(&mut normal, observer, ObjectId(1), Some(target)).unwrap();
    for world in [&mut normal, &mut observing] {
        set_battle_map_visibility(world, map, BattleLight::Day, 0).unwrap();
    }
    let normal_events = refresh_optical_scanners(&mut normal, &[observer]).unwrap();
    let events = refresh_optical_scanners(&mut observing, &[observer]).unwrap();
    assert_eq!(events, normal_events);
    let lost = events.iter().find(|e| e.target == target).unwrap();
    assert!(lost.lock_lost);
    assert_eq!(
        lost.notice(&observing).unwrap().text,
        "Weapon system reports the lock has been lost."
    );
    let notice = lost.notice(&normal).unwrap().text;
    assert!(notice.starts_with("You have lost "));
    assert!(notice.ends_with("Weapon system reports the lock has been lost."));
    assert!(
        observing.btech.constructed_units()[&observer]
            .target_lock()
            .is_none()
    );
    assert!(
        observing.btech.constructed_units()[&observer]
            .contacts()
            .is_empty()
    );
    let mut compared = observing.clone();
    set_battle_observer(&mut compared, observer, false).unwrap();
    assert_eq!(compared.btech, normal.btech);
    persistence::save(&config.database(), &observing)
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(lost.notice(&restored), lost.notice(&observing));
}

#[tokio::test]
async fn observer_contact_notice_policy_preserves_lock_warning_only() {
    let (_dir, _config, mut world, unit) = fixture().await;
    for acquired in [false, true] {
        let event = BattleContactEvent {
            experience_message: None,
            identified: true,
            observer: unit,
            target: ObjectId(99),
            acquired,
            lock_lost: false,
        };
        // An unavailable target cannot supply a routine identity/arc notice.
        assert!(event.notice(&world).is_none());
        set_battle_observer(&mut world, unit, true).unwrap();
        assert!(event.notice(&world).is_none());
        assert_eq!(
            BattleContactEvent {
                lock_lost: true,
                ..event.clone()
            }
            .notice(&world)
            .unwrap()
            .text,
            "Weapon system reports the lock has been lost."
        );
        set_battle_observer(&mut world, unit, false).unwrap();
    }
    assert!(
        BattleContactEvent {
            experience_message: None,
            identified: true,
            observer: ObjectId(-1),
            target: unit,
            acquired: false,
            lock_lost: true
        }
        .notice(&world)
        .is_none()
    );
}

/// Explicit configuration owns channel count and capabilities, independently of radio quality.
#[test]
fn radio_type_decodes_capabilities_and_gates_modes() {
    for (configuration, channels, relay, info, scan, digital) in [
        (0, 4, false, false, false, true),
        (126, 14, true, true, true, true),
        (133, 5, false, false, false, false),
        (16, 0, true, false, false, true),
        (255, 15, true, true, true, false),
    ] {
        let mut template =
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        template
            .attributes
            .insert("radiotype".into(), configuration.to_string());
        let unit = BattleUnit::from_template(template).unwrap();
        let capabilities = unit.radio_capabilities();
        assert_eq!(
            capabilities,
            BattleRadioCapabilities {
                channels,
                range: 80,
                relay,
                info,
                scan,
                digital
            }
        );
        assert_eq!(unit.radio_channels().len(), usize::from(channels));
        assert_eq!(BattleRadioMode::parse("D", capabilities).is_ok(), digital);
        assert_eq!(
            BattleRadioMode::parse("DI", capabilities).is_ok(),
            digital && info
        );
        assert_eq!(BattleRadioMode::parse("S", capabilities).is_ok(), scan);
        assert!(BattleRadioMode::parse("I", capabilities).is_err());
    }
    for value in ["-1", "256", "invalid"] {
        let mut template =
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        template.attributes.insert("radiotype".into(), value.into());
        assert!(BattleUnit::from_template(template).is_err());
    }
}

/// Receiver info renders the relay hops; analog-only hardware cannot hear digital traffic.
#[tokio::test]
async fn radio_type_info_and_digital_exclusion_follow_relay_routing() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    let source = units[0];
    let receiver = units[4];
    radio_fact(&mut world, receiver, |u| {
        u["definition"]["attributes"]["radiotype"] = "126".into();
        u["radio"][0]["mode"]["digital"] = true.into();
        u["radio"][0]["mode"]["info"] = true.into();
    });
    let report = resolve_digital_radio(&world, source, 0, "Info").unwrap();
    let reception = report
        .receptions
        .iter()
        .find(|r| r.receiver == receiver)
        .unwrap();
    let text = text::plain(&reception.text);
    assert!(text.contains("{R-path:"));
    for &relay in &reception.transmitters[1..] {
        assert!(text.contains(
            &format!("[{}]-h:",world.btech.constructed_units()[&relay].battlefield_id().unwrap())
        ));
    }
    assert!(
        !report
            .receptions
            .iter()
            .find(|r| r.receiver == source)
            .unwrap()
            .text
            .contains("R-path")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_digital_radio(&restored, source, 0, "Info").unwrap(),
        report
    );
    radio_fact(&mut world, receiver, |u| {
        u["definition"]["attributes"]["radiotype"] = "142".into()
    });
    assert!(
        !resolve_digital_radio(&world, source, 0, "Info")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == receiver)
    );
}

/// Unmatched analog traffic searches on saved receiver dice, including muted shutdown scanners.
#[tokio::test]
async fn frequency_scanning_replays_and_rolls_back_with_transmission() {
    let (_dir, config, mut world, _map, units) = relay_fixture().await;
    let source = units[0];
    let receiver = units[4];
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = false.into();
        u["pilot"] = 1.into();
    });
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(source);
    radio_fact(&mut world, receiver, |u| {
        u["definition"]["attributes"]["radiotype"] = "66".into();
        u["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        for (index, frequency) in [(0, 0), (1, 100)] {
            u["radio"][index]["frequency"] = frequency.into();
            u["radio"][index]["mode"]["scan"] = true.into();
            u["radio"][index]["mode"]["muted"] = true.into();
        }
    });
    let message = "x".repeat(80);
    let mut selected = None;
    for seed in 0..=255 {
        radio_fact(&mut world, receiver, |u| {
            u["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        let mut candidate = world.clone();
        let report = resolve_analog_radio(&mut candidate, source, 0, &message).unwrap();
        if report.scans.len() == 2 {
            selected = Some((candidate, report));
            break;
        }
    }
    let (after, report) = selected.unwrap();
    assert_eq!(report.scans[0].receiver, receiver);
    assert!(report.scans[0].frequency > 0 && report.scans[0].frequency <= 42);
    assert!(report.scans[1].frequency >= 42 && report.scans[1].frequency < 100);
    assert_eq!(
        report
            .notices()
            .iter()
            .filter(|n| n.unit == receiver)
            .count(),
        3
    );
    assert!(!report.receptions.iter().any(|r| r.receiver == receiver));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_analog_radio(&mut restored, source, 0, &message).unwrap(),
        report
    );
    assert_eq!(restored.btech, after.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<mlua::Value>(&format!(
                "assert(btech.unit.radio_send({},1,0,string.rep('x',80))); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let sent = send_radio_action(&scripts, &config, source, ObjectId(1), 0, &message).unwrap();
    let BattleRadioDelivery::Analog(delivery) = sent.delivery else {
        panic!()
    };
    assert_eq!(delivery, report);
    assert_eq!(scripts.world().btech, after.btech);
}

/// Native and Lua controls expose info/scanning across the expanded channel bank.
#[tokio::test]
async fn extended_radio_modes_native_lua_and_scan_exclusions() {
    let (_dir, config, mut world, id) = fixture().await;
    radio_fact(&mut world, id, |u| {
        u["definition"]["attributes"]["radiotype"] = "126".into()
    });
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let output = support::run_text(&native, &config, ObjectId(1), 1, "setchannelmode N=DIUS");
    assert!(output.contains("flags:IUS"));
    lua.eval_callback::<mlua::Value>(&format!(
        "return assert(btech.unit.radio_mode({},1,13,'DIUS'))",
        id.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let output = support::run_text(&native, &config, ObjectId(1), 1, "listfreqs");
    assert!(output.contains("N    D-MS"));
    let before = native.world().btech.clone();
    assert!(
        set_radio_mode(
            &mut native.world_mut(),
            id,
            ObjectId(1),
            14,
            BattleRadioMode::default()
        )
        .is_err()
    );
    assert_eq!(native.world().btech, before);

    let (_dir, _config, mut world, _map, units) = relay_fixture().await;
    let source = units[0];
    let receiver = units[4];
    radio_fact(&mut world, receiver, |u| {
        u["definition"]["attributes"]["radiotype"] = "65".into();
        u["radio"][0]["mode"]["scan"] = true.into();
        u["radio"][0]["frequency"] = 43.into();
    });
    let before = world.btech.clone();
    let digital = resolve_digital_radio(&world, source, 0, &"x".repeat(100)).unwrap();
    assert!(!digital.receptions.iter().any(|r| r.receiver == receiver));
    assert_eq!(world.btech, before);
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["mode"]["digital"] = false.into();
        u["radio"][0]["frequency"] = 0.into();
    });
    let unchanged = world.btech.constructed_units()[&receiver].clone();
    let report = resolve_analog_radio(&mut world, source, 0, &"x".repeat(100)).unwrap();
    assert!(report.scans.is_empty());
    assert_eq!(world.btech.constructed_units()[&receiver], unchanged);
    radio_fact(&mut world, source, |u| {
        u["radio"][0]["frequency"] = 43.into()
    });
    let report = resolve_analog_radio(&mut world, source, 0, &"x".repeat(100)).unwrap();
    assert!(report.scans.is_empty());
    assert!(report.receptions.iter().any(|r| r.receiver == receiver));
}

/// Native lexical errors differ from tunable-range errors and never trigger frequency audits.
#[tokio::test]
async fn native_frequency_parser_matches_reference_decimal_boundaries() {
    let (_dir, config, world, id) = fixture().await;
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for (input, expected) in [
        ("-1", "Invalid frequency!"),
        ("-0", "Invalid frequency!"),
        ("+1", "Invalid frequency!"),
        ("1.0", "Invalid frequency!"),
        ("1e2", "Invalid frequency!"),
        ("１２", "Invalid frequency!"),
        ("1000000", "Invalid frequency - range is from 0 to 999999."),
        (
            "2147483647",
            "Invalid frequency - range is from 0 to 999999.",
        ),
        ("2147483648", "Invalid frequency!"),
        ("4294967295", "Invalid frequency!"),
        ("999999999999999999999999", "Invalid frequency!"),
    ] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("setchannelfreq A={input}")
            ),
            expected
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    for (input, frequency) in [("0", 0), ("000001", 1), ("999999", 999999)] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("setchannelfreq A={input}")
            ),
            format!("Channel A set to {frequency}.")
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].radio_channels()[0].frequency,
            frequency
        );
    }
}
