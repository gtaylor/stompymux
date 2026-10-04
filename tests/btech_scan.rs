//! Detailed scan disclosure, hardware ranges, observer privileges and native/Lua parity.
use crate::support;
use stompymux_rs::*;

/// A piloted running scanner and an acquired shutdown target on a clear north/south map.
async fn fixture() -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    fixture_with_ranges(&[]).await
}

/// Construct both units using the same optional template range settings.
async fn fixture_with_ranges(
    ranges: &[(&str, &str)],
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Scan field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "scan.map",
        BattleMapAsset::from_cells(&format!("3 60\n{}", ".0.0.0\n".repeat(60))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let mut units = Vec::new();
    for y in [1, 2] {
        let id = world.create(&config, "Scan unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mut template =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        for &(field, value) in ranges {
            template.attributes.insert(field.into(), value.into());
        }
        create_battle_unit(&mut world, id, template).unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, map, 1, y).unwrap();
        units.push(id);
    }
    let source = units[0];
    let target = units[1];
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(source);
    assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    acquire(&mut world, source, target);
    (dir, config, world, map, source, target)
}

/// Leave no channel that reaches anything: the sensor band switched off and no visibility.
fn blind(world: &mut World, map: ObjectId) {
    set_battle_map_perception(world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    set_battle_map_visibility(world, map, BattleLight::Day, 0).unwrap();
}

/// Restore the fixture's clear day with the sensor band switched back on.
fn unblind(world: &mut World, map: ObjectId) {
    set_battle_map_perception(world, map, BattleMapPerceptionFlag::Sensors, true).unwrap();
    set_battle_map_visibility(world, map, BattleLight::Day, 30).unwrap();
}

/// Save a known contact without consuming acquisition dice.
fn acquire(world: &mut World, source: ObjectId, target: ObjectId) {
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["contacts"][target.0.to_string()] = serde_json::json!({"identified": false});
        })
        .unwrap();
}

#[test]
fn sensor_defaults_follow_technology_base_and_critical_halving() {
    for (clan, base) in [(false, 25), (true, 35)] {
        let mut template =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        for field in ["tac_range", "lrs_range", "scan_range"] {
            template.attributes.remove(field);
        }
        if clan {
            template
                .attributes
                .insert("specials".into(), "Clan FlipArms".into());
            template.heat_sinks = 20;
            for section in template.sections.values_mut() {
                section
                    .criticals
                    .retain(|_, part| part.equipment != "HeatSink");
            }
        }
        let mut unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(
            unit.sensor_ranges(),
            BattleSensorRanges {
                tactical: base,
                long_range: base * 2,
                scan: base
            }
        );
        unit.destroy_critical(CriticalLocation {
            section: BattleSection::Head,
            slot: 1,
        })
        .unwrap();
        assert_eq!(
            unit.sensor_ranges(),
            BattleSensorRanges {
                tactical: base / 2,
                long_range: base,
                scan: base / 2
            }
        );
        unit.destroy_critical(CriticalLocation {
            section: BattleSection::Head,
            slot: 4,
        })
        .unwrap();
        assert_eq!(unit.sensor_ranges().scan, 0);
    }
}

#[tokio::test]
async fn detailed_scan_limits_information_and_matches_native_lua() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    let before = world.btech.clone();
    let ordinary = scan_battle_unit(&world, source, ObjectId(1), target, "").unwrap();
    assert!(ordinary.contains("FRONT") && ordinary.contains("WEAPON SYSTEMS"));
    assert!(text::plain(&ordinary).contains("__(OO)__"), "{ordinary}");
    assert!(
        text::plain(&ordinary)
            .lines()
            .any(|line| line.starts_with("Key"))
    );
    assert!(!ordinary.contains("ammunition"));
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan AB");
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.scan({},1,{})",
            source.0, target.0
        ))
        .unwrap();
    assert_eq!(native, lua);
    assert_eq!(lua, ordinary);
    assert_eq!(scripts.world().btech, before);
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "scan A"),
        scan_battle_unit(&world, source, ObjectId(1), target, "A").unwrap()
    );
    set_battle_observer(&mut world, source, true).unwrap();
    let privileged = scan_battle_unit(&world, source, ObjectId(1), target, "").unwrap();
    assert!(privileged.contains("AMMUNITION"), "{privileged}");
    assert!(!privileged.contains("O >90%"));
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        scan_battle_unit(&restored, source, ObjectId(1), target, "").unwrap(),
        privileged
    );
}

#[tokio::test]
async fn observer_scan_bypasses_range_but_not_visibility_or_failed_hardware() {
    let (_dir, _config, mut world, map, source, target) = fixture().await;
    place_battle_unit(&mut world, target, map, 1, 22).unwrap();
    acquire(&mut world, source, target);
    assert!(
        visible_battle_contact(&world, source, target)
            .unwrap()
            .is_some()
    );
    assert!(
        scan_battle_unit(&world, source, ObjectId(1), target, "I")
            .unwrap_err()
            .to_string()
            .contains("scanner range")
    );
    set_battle_observer(&mut world, source, true).unwrap();
    assert!(scan_battle_unit(&world, source, ObjectId(1), target, "I").is_ok());
    assert!(scan_battle_unit(&world, source, ObjectId(2), target, "I").is_err());
    assert!(scan_battle_unit(&world, source, ObjectId(1), target, "N").is_err());
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    assert!(scan_battle_unit(&world, source, ObjectId(1), target, "I").is_err());
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["lost_criticals"] = serde_json::json!([
                {"section": "Head", "slot": 1}, {"section": "Head", "slot": 4}
            ]);
        })
        .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&source]
            .sensor_ranges()
            .scan,
        0
    );
    assert!(
        scan_battle_unit(&world, source, ObjectId(1), target, "I")
            .unwrap_err()
            .to_string()
            .contains("inoperational")
    );
}

#[tokio::test]
async fn scan_warnings_follow_target_visibility_and_rollback_with_lua() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    for identified in [false, true] {
        if identified {
            acquire(&mut world, target, source);
        }
        let expected = if identified {
            "You are being scanned by Jenner [aa]"
        } else {
            "You are being scanned by something [AA]"
        };
        let shared = std::rc::Rc::new(std::cell::RefCell::new(world.clone()));
        let scripts = Scripts::new(&config, shared.clone()).unwrap();
        let before = shared.borrow().btech.clone();
        let query = scan_battle_unit(&shared.borrow(), source, ObjectId(1), target, "A").unwrap();
        assert!(scripts.drain_outbox().is_empty());
        let report: String = scripts
            .eval_callback(&format!(
                "return btech.unit.scan({},1,{},'A')",
                source.0, target.0
            ))
            .unwrap();
        assert_eq!(query, report);
        let notices = scripts.drain_outbox();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].0, ObjectId(2));
        assert_eq!(notices[0].1.source(), expected);
        assert_eq!(shared.borrow().btech, before);
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan AB A");
        assert!(native.contains(&report));
        assert_eq!(native.matches(expected).count(), 1);
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.scan({},1,{},'A'); error('abort scan')",
                    source.0, target.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        assert!(scan_battle_unit_action(&scripts, source, ObjectId(1), target, "N").is_err());
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(shared.borrow().btech, before);
        set_battle_observer(&mut shared.borrow_mut(), source, true).unwrap();
        assert!(scan_battle_unit_action(&scripts, source, ObjectId(1), target, "A").is_ok());
        assert!(scripts.drain_outbox().is_empty());
        set_battle_observer(&mut shared.borrow_mut(), source, false).unwrap();
        let mut state = serde_json::to_value(&shared.borrow().btech).unwrap();
        state["constructed"][target.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Off).unwrap();
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        assert!(scan_battle_unit_action(&scripts, source, ObjectId(1), target, "A").is_ok());
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// Coordinate scans pick acquired occupants in saved order and fail once nothing reaches the hex.
#[tokio::test]
async fn coordinate_scan_selects_visible_occupants_in_saved_order() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    let other = world.create(&config, "Second occupant".into(), Kind::Thing);
    world.objects.get_mut(&other).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        other,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, other, map, 1, 2).unwrap();
    acquire(&mut world, source, other);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    let before = shared.borrow().btech.clone();
    let report =
        scan_battle_hex_unit_action(&scripts, source, ObjectId(1), coordinate, "").unwrap();
    assert_eq!(
        report,
        scan_battle_unit(&shared.borrow(), source, ObjectId(1), target, "").unwrap()
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "scan 1 2"),
        report
    );
    let lua: String = scripts
        .eval_callback(&format!("return btech.unit.scan_hex({},1,1,2)", source.0))
        .unwrap();
    assert_eq!(lua, report);
    assert_eq!(shared.borrow().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let empty = scan_battle_hex_unit_action(
        &scripts,
        source,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 1 },
        "",
    )
    .unwrap();
    assert_eq!(empty, "You see nobody in the hex!");
    let mut state = serde_json::to_value(&shared.borrow().btech).unwrap();
    state["constructed"][source.0.to_string()]["contacts"]
        .as_object_mut()
        .unwrap()
        .remove(&target.0.to_string());
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let second =
        scan_battle_hex_unit_action(&scripts, source, ObjectId(1), coordinate, "").unwrap();
    assert_eq!(
        second,
        scan_battle_unit(&shared.borrow(), source, ObjectId(1), other, "").unwrap()
    );
    let snapshot = shared.borrow().clone();
    persistence::save(&config.database(), &snapshot)
        .await
        .unwrap();
    let restored = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    assert_eq!(
        scan_battle_hex_unit_action(&restored, source, ObjectId(1), coordinate, "").unwrap(),
        second
    );
    let mut state = serde_json::to_value(&shared.borrow().btech).unwrap();
    state["constructed"][source.0.to_string()]["contacts"] = serde_json::json!({});
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        scan_battle_hex_unit_action(&scripts, source, ObjectId(1), coordinate, "").unwrap(),
        empty
    );
    for coordinate in [
        BattleHexCoordinate { x: -1, y: 2 },
        BattleHexCoordinate { x: 3, y: 2 },
        BattleHexCoordinate { x: 1, y: 22 },
    ] {
        assert!(
            scan_battle_hex_unit_action(&scripts, source, ObjectId(1), coordinate, "").is_err()
        );
    }
    assert!(scan_battle_hex_unit_action(&scripts, source, ObjectId(2), coordinate, "").is_err());
    blind(&mut shared.borrow_mut(), map);
    assert!(scan_battle_hex_unit_action(&scripts, source, ObjectId(1), coordinate, "").is_err());
}

/// Register an entrance whose interior owns the construction integrity.
fn scan_structure(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let interior = world.create(config, "Hangar".into(), Kind::Room);
    create_battle_map(
        world,
        interior,
        "hangar.map",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, interior, support::FIXTURE_DICE_SEED);
    set_building_state(
        world,
        interior,
        BattleBuildingState {
            integrity: 31,
            maximum_integrity: 50,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    set_building_entrance(
        world,
        map,
        0,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 1, y: 2 },
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    interior
}

#[tokio::test]
async fn building_scans_report_integrity_and_hide_unavailable_interiors() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    let interior = scan_structure(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    let before = world.btech.clone();
    let report = scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1000).unwrap();
    assert_eq!(report.text, "The Hangar's CF is 31.");
    assert!(report.experience_messages.is_empty());
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "scan 1 2 B"),
        report.text
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.scan_building({},1,1,2).text",
            source.0
        ))
        .unwrap();
    assert_eq!(lua, report.text);
    let notices = scripts.drain_outbox();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].1.source(), report.text);
    let empty = scan_battle_building(
        &mut world,
        source,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 3 },
        1000,
    )
    .unwrap();
    for flags in [4, 16] {
        let mut state = world.btech.maps()[&interior].building;
        state.flags = flags;
        set_building_state(&mut world, interior, state).unwrap();
        let before = world.btech.clone();
        assert_eq!(
            scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1000).unwrap(),
            empty
        );
        assert_eq!(world.btech, before);
    }
    world
        .objects
        .get_mut(&interior)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert_eq!(
        scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1000).unwrap(),
        empty
    );
    assert!(scan_battle_building(&mut world, source, ObjectId(2), coordinate, 1000).is_err());
    set_battle_observer(&mut world, source, true).unwrap();
    assert!(
        scan_battle_building(
            &mut world,
            source,
            ObjectId(1),
            BattleHexCoordinate { x: 1, y: 22 },
            1000
        )
        .is_err()
    );
}

#[tokio::test]
async fn concealed_building_rolls_awards_and_callback_rollback_replay() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    let interior = scan_structure(&mut world, &config, map);
    let mut building = world.btech.maps()[&interior].building;
    building.flags = 4;
    set_building_state(&mut world, interior, building).unwrap();
    world
        .objects
        .get_mut(&source)
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
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    // The startup-captured target of 18 cannot succeed, but an eligible attempt spends two dice.
    let mut expected = world.clone();
    roll_unit_dice(&mut expected, source, 2).unwrap();
    let missed = scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1000).unwrap();
    assert!(missed.text.contains("no building"));
    assert_eq!(world.btech, expected.btech);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["scanner_perception"] = (-10).into();
        })
        .unwrap();
    let checkpoint = world.clone();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world.clone()));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.scan_building({},1,1,2); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(shared.borrow().btech, checkpoint.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    restored
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let report = scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1000).unwrap();
    assert_eq!(report.text, "The Hangar's CF is 31.");
    assert_eq!(report.experience_messages.len(), 1);
    assert_eq!(
        report.experience_messages[0].text,
        "GOD gained 1 perception XP"
    );
    assert_eq!(
        world.btech.character_values()[&ObjectId(1)]["Perception"].experience_balance(),
        1
    );
    assert_eq!(
        scan_battle_building(&mut restored, source, ObjectId(1), coordinate, 1000).unwrap(),
        report
    );
    // Restart restores the referenced damaged building's missing repair interval.
    let mut expected = serde_json::to_value(&world.btech).unwrap();
    expected["maps"][interior.0.to_string()]["building_repair"] = 120.into();
    assert_eq!(serde_json::to_value(&restored.btech).unwrap(), expected);
    assert!(
        scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1001)
            .unwrap()
            .experience_messages
            .is_empty()
    );
    building.flags = 16;
    set_building_state(&mut world, interior, building).unwrap();
    let before = world.btech.clone();
    assert!(
        scan_battle_building(&mut world, source, ObjectId(1), coordinate, 1031)
            .unwrap()
            .text
            .contains("no building")
    );
    assert_eq!(world.btech, before);
}

/// Install a field without triggering it or requiring unit movement.
fn scan_mine(world: &mut World, map: ObjectId, coordinate: BattleHexCoordinate) {
    set_minefield(
        world,
        map,
        0,
        Some(BattleMinefield {
            coordinate,
            kind: BattleMineKind::Command,
            strength: 12,
            extra: 123,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
}

/// Enable a guaranteed successful perception target without advancing gameplay during a scan test.
fn scan_perception(world: &mut World, source: ObjectId) {
    world
        .objects
        .get_mut(&source)
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
    support::seed_object_dice(world, ObjectId(1), support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["scanner_perception"] = (-10).into();
        })
        .unwrap();
}

#[tokio::test]
async fn mine_recognition_gates_dice_and_replays_without_revealing_configuration() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    let before = world.btech.clone();
    let empty = scan_battle_mines(&mut world, source, ObjectId(1), coordinate, 1000).unwrap();
    assert!(!empty.found);
    assert_eq!(world.btech, before);
    scan_mine(&mut world, map, coordinate);
    let mut expected = serde_json::to_value(&world.btech).unwrap();
    let mut dice: BattleDice =
        serde_json::from_value(expected["constructed"][source.0.to_string()]["dice"].clone())
            .unwrap();
    dice.die(8).unwrap();
    expected["constructed"][source.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
    assert_eq!(
        scan_battle_mines(&mut world, source, ObjectId(1), coordinate, 1000).unwrap(),
        empty
    );
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), expected);
    scan_perception(&mut world, source);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    restored
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let report = scan_battle_mines(&mut world, source, ObjectId(1), coordinate, 1000).unwrap();
    assert!(report.found);
    assert_eq!(report.experience_messages.len(), 1);
    assert!(!report.text.contains("123") && !report.text.contains("Command"));
    assert_eq!(
        scan_battle_mines(&mut restored, source, ObjectId(1), coordinate, 1000).unwrap(),
        report
    );
    assert_eq!(world.btech, restored.btech);
    assert_eq!(world.btech.maps()[&map].minefields().len(), 1);
    let far = BattleHexCoordinate { x: 1, y: 11 };
    scan_mine(&mut world, map, far);
    let mut expected = serde_json::to_value(&world.btech).unwrap();
    let mut dice: BattleDice =
        serde_json::from_value(expected["constructed"][source.0.to_string()]["dice"].clone())
            .unwrap();
    dice.die(8).unwrap();
    expected["constructed"][source.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
    assert_eq!(
        scan_battle_mines(&mut world, source, ObjectId(1), far, 1031).unwrap(),
        empty
    );
    assert_eq!(serde_json::to_value(&world.btech).unwrap(), expected);
}

#[tokio::test]
async fn combined_hex_scan_routes_private_failure_and_rolls_back_both_phases() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    scan_structure(&mut world, &config, map);
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    scan_mine(&mut world, map, coordinate);
    for player in [ObjectId(1), ObjectId(2)] {
        world.objects.get_mut(&player).unwrap().location = Some(source);
        world
            .objects
            .get_mut(&player)
            .unwrap()
            .flags
            .insert(Flag::Connected);
    }
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world.clone()));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let failed =
        scan_battle_hex_action(&scripts, &config, source, ObjectId(1), coordinate).unwrap();
    assert!(!failed.mines.found);
    let notices = scripts.drain_outbox();
    assert_eq!(
        notices.iter().filter(|(id, _)| *id == ObjectId(1)).count(),
        2
    );
    assert_eq!(
        notices.iter().filter(|(id, _)| *id == ObjectId(2)).count(),
        1
    );
    scan_perception(&mut world, source);
    *shared.borrow_mut() = world.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.scan_terrain({},1,1,2); error('abort hex scan')",
                source.0
            ))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(shared.borrow().btech, world.btech);
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan 1 2 H");
    assert_eq!(native.matches("The Hangar's CF is 31.").count(), 2);
    assert_eq!(native.matches("Small bomblets").count(), 2);
    *shared.borrow_mut() = world.clone();
    let found: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.scan_terrain({},1,1,2).mines.found",
            source.0
        ))
        .unwrap();
    assert!(found);
    let lua = scripts
        .drain_outbox()
        .iter()
        .map(|(_, text)| text.source().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(native, lua);
    assert_eq!(
        shared.borrow().btech.maps()[&map].minefields(),
        world.btech.maps()[&map].minefields()
    );
}

#[tokio::test]
async fn selected_scan_dispatches_each_lock_mode_without_changing_countdowns() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    scan_structure(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world.clone()));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    assert!(scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").is_err());
    for mode in [
        BattleHexTargetMode::UnitAtHex,
        BattleHexTargetMode::Ignite,
        BattleHexTargetMode::Clear,
        BattleHexTargetMode::Building,
        BattleHexTargetMode::Hex,
    ] {
        let _ =
            select_battle_hex_target(&mut world, source, ObjectId(1), coordinate, mode).unwrap();
        *shared.borrow_mut() = world.clone();
        let before = world.btech.clone();
        let report =
            scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").unwrap();
        match (&report, mode) {
            (BattleSelectedScan::Building(report), BattleHexTargetMode::Building) => {
                assert!(report.text.contains("CF is 31"))
            }
            (BattleSelectedScan::Hex(report), BattleHexTargetMode::Hex) => {
                assert!(!report.mines.found)
            }
            (BattleSelectedScan::Unit(text), _) => assert_eq!(
                *text,
                scan_battle_unit(&world, source, ObjectId(1), target, "").unwrap()
            ),
            _ => panic!("Wrong selected scan kind"),
        }
        scripts.drain_outbox();
        assert_eq!(shared.borrow().btech, before);
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "scan");
        let lua: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.scan_selected({},1)", source.0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(&report).unwrap()
        );
        let messages = scripts
            .drain_outbox()
            .iter()
            .map(|(_, text)| text.source().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        match report {
            BattleSelectedScan::Unit(text) => assert_eq!(native, text),
            _ => assert_eq!(native, messages),
        }
        assert_eq!(shared.borrow().btech, before);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.constructed_units()[&source].hex_lock(),
        world.btech.constructed_units()[&source].hex_lock()
    );
    *shared.borrow_mut() = restored;
    assert!(matches!(
        scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").unwrap(),
        BattleSelectedScan::Hex(_)
    ));
}

#[tokio::test]
async fn selected_observer_coordinates_bypass_distance_but_keep_visibility_and_rollback() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    let interior = scan_structure(&mut world, &config, map);
    let far = BattleHexCoordinate { x: 1, y: 22 };
    set_building_entrance(
        &mut world,
        map,
        1,
        Some(BattleBuildingEntrance {
            coordinate: far,
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    let _ = select_battle_hex_target(
        &mut world,
        source,
        ObjectId(1),
        far,
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    assert!(scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").is_err());
    set_battle_observer(&mut shared.borrow_mut(), source, true).unwrap();
    let report = scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").unwrap();
    assert!(matches!(report, BattleSelectedScan::Hex(_)));
    assert!(scan_battle_hex_action(&scripts, &config, source, ObjectId(1), far).is_err());
    set_battle_map_visibility(&mut shared.borrow_mut(), map, BattleLight::Day, 0).unwrap();
    assert!(scan_battle_selected_action(&scripts, &config, source, ObjectId(1), "").is_err());
    set_battle_map_visibility(&mut shared.borrow_mut(), map, BattleLight::Day, 30).unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    scan_mine(&mut shared.borrow_mut(), map, coordinate);
    scan_perception(&mut shared.borrow_mut(), source);
    let _ = select_battle_hex_target(
        &mut shared.borrow_mut(),
        source,
        ObjectId(1),
        coordinate,
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let _ = scripts.drain_outbox();
    let before = shared.borrow().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.scan_selected({},1); error('abort selected')",
                source.0
            ))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(shared.borrow().btech, before);
}

#[tokio::test]
async fn brief_reports_are_silent_and_do_not_use_detailed_scan_range() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    place_battle_unit(&mut world, target, map, 1, 22).unwrap();
    acquire(&mut world, source, target);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let before = world.btech.clone();
    assert!(scan_battle_unit(&world, source, ObjectId(1), target, "").is_err());
    let report = report_battle_unit(&world, source, ObjectId(1), target).unwrap();
    assert!(report.contains("Jenner") && report.contains("Range: 21.0"));
    assert!(
        !report.contains("FRONT")
            && !report.contains("WEAPON SYSTEMS")
            && !report.contains("ammunition")
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report AB"),
        report
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report"),
        report
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.report({},1,{})",
            source.0, target.0
        ))
        .unwrap();
    assert_eq!(lua, report);
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report 1 22")
            .contains("out of scanner range")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        report_battle_unit(&restored, source, ObjectId(1), target).unwrap(),
        report
    );
    assert!(report_battle_unit(&world, source, ObjectId(2), target).is_err());
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    assert!(report_battle_unit(&world, source, ObjectId(1), target).is_err());
}

#[tokio::test]
async fn brief_coordinate_reports_follow_visible_occupants_and_saved_hex_selection() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    let _ = select_battle_hex_target(
        &mut world,
        source,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 2 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let report = report_battle_unit(&world, source, ObjectId(1), target).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world.clone()));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report 1 2"),
        report
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report"),
        report
    );
    assert_eq!(shared.borrow().btech, world.btech);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report 1 3"),
        "No target found."
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][source.0.to_string()]["contacts"] = serde_json::json!({});
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report 1 2"),
        "No target found."
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "report"),
        "You don't see a thing."
    );
}

#[tokio::test]
async fn display_centers_project_signed_ranges_and_share_lua_grammar() {
    let (_dir, config, world, _map, source, target) = fixture().await;
    let before = world.btech.clone();
    let own = resolve_battle_view_center(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        BattleViewCenter::OwnUnit,
    )
    .unwrap();
    assert_eq!(own.center, BattleHexCoordinate { x: 1, y: 1 });
    assert_eq!(own.maximum_range, 20);
    let forward = parse_battle_view_center(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        "180 20.9",
    )
    .unwrap();
    let backward = parse_battle_view_center(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        "0 -20.9",
    )
    .unwrap();
    assert_eq!(forward, backward);
    assert_eq!(forward.center, BattleHexCoordinate { x: 1, y: 22 });
    assert_eq!(
        parse_battle_view_center(
            &world,
            source,
            ObjectId(1),
            BattleViewKind::Tactical,
            "540 20.9"
        )
        .unwrap(),
        forward
    );
    let contact =
        parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::Tactical, "ab")
            .unwrap();
    assert_eq!(contact.center, BattleHexCoordinate { x: 1, y: 2 });
    assert_eq!(
        parse_battle_view_center(
            &world,
            source,
            ObjectId(1),
            BattleViewKind::Tactical,
            &format!("#{}", target.0)
        )
        .unwrap(),
        contact
    );
    for args in ["180 21", "0 -21", "90 NaN", "0 inf", "0 1 2", "2.5 1"] {
        assert!(
            parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::Tactical, args)
                .is_err(),
            "{args}"
        );
    }
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.view_center({},1,'tactical','180 20.9')",
            source.0
        ))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(forward).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        parse_battle_view_center(
            &restored,
            source,
            ObjectId(1),
            BattleViewKind::Tactical,
            "180 20.9"
        )
        .unwrap(),
        forward
    );
}

#[tokio::test]
async fn display_center_observer_exemption_is_projection_only() {
    let (_dir, _config, mut world, map, source, target) = fixture().await;
    place_battle_unit(&mut world, target, map, 1, 22).unwrap();
    acquire(&mut world, source, target);
    set_battle_observer(&mut world, source, true).unwrap();
    assert!(
        parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::Tactical, "AB")
            .is_err()
    );
    let long =
        parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::LongRange, "AB")
            .unwrap();
    assert_eq!(long.maximum_range, 40);
    let projected = parse_battle_view_center(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        "180 100",
    )
    .unwrap();
    assert_eq!(projected.center, BattleHexCoordinate { x: 1, y: 101 });
    assert!(
        parse_battle_view_center(
            &world,
            source,
            ObjectId(1),
            BattleViewKind::Tactical,
            "180 1e100"
        )
        .is_err()
    );
    assert!(
        parse_battle_view_center(&world, source, ObjectId(2), BattleViewKind::Tactical, "")
            .is_err()
    );
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    assert!(
        parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::LongRange, "AB")
            .is_err()
    );
    // Center projection reveals coordinates only; terrain and occupants need separate display filtering.
    assert!(
        parse_battle_view_center(
            &world,
            source,
            ObjectId(1),
            BattleViewKind::Tactical,
            "180 100"
        )
        .is_ok()
    );
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["lost_criticals"] = serde_json::json!([
                {"section": "Head", "slot": 1}, {"section": "Head", "slot": 4}
            ]);
        })
        .unwrap();
    assert!(
        parse_battle_view_center(&world, source, ObjectId(1), BattleViewKind::LongRange, "")
            .is_err()
    );
}

#[tokio::test]
async fn viewports_clip_requested_dimensions_and_match_lua_without_state_changes() {
    let (_dir, config, mut world, _map, source, _) = fixture().await;
    let dimensions = BattleViewDimensions {
        tactical_width: 40,
        tactical_height: 24,
        long_range_height: 40,
    };
    let tactical = resolve_battle_viewport(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        "",
        dimensions,
    )
    .unwrap();
    assert_eq!((tactical.width, tactical.height), (3, 24));
    assert_eq!(tactical.origin, BattleHexCoordinate { x: 0, y: 0 });
    let long = resolve_battle_viewport(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::LongRange,
        "",
        dimensions,
    )
    .unwrap();
    assert_eq!((long.width, long.height), (3, 41));
    set_battle_observer(&mut world, source, true).unwrap();
    let far = resolve_battle_viewport(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::LongRange,
        "180 100",
        dimensions,
    )
    .unwrap();
    assert_eq!(far.origin, BattleHexCoordinate { x: 0, y: 19 });
    assert_eq!(far.requested_center, BattleHexCoordinate { x: 1, y: 101 });
    let before = world.btech.clone();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.viewport({},1,'long_range','180 100',{{long_range_height=40}})",
            source.0
        ))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(far).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    for invalid in [
        BattleViewDimensions {
            tactical_width: 4,
            ..dimensions
        },
        BattleViewDimensions {
            tactical_height: 25,
            ..dimensions
        },
        BattleViewDimensions {
            long_range_height: 41,
            ..dimensions
        },
    ] {
        assert!(
            resolve_battle_viewport(
                &world,
                source,
                ObjectId(1),
                BattleViewKind::Tactical,
                "",
                invalid
            )
            .is_err()
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        resolve_battle_viewport(
            &restored,
            source,
            ObjectId(1),
            BattleViewKind::LongRange,
            "180 100",
            dimensions
        )
        .unwrap(),
        far
    );
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["lost_criticals"] = serde_json::json!([{ "section": "Head", "slot": 1 }]);
        })
        .unwrap();
    let damaged = resolve_battle_viewport(
        &world,
        source,
        ObjectId(1),
        BattleViewKind::Tactical,
        "",
        dimensions,
    )
    .unwrap();
    assert_eq!(
        (damaged.maximum_range, damaged.width, damaged.height),
        (10, 3, 20)
    );
}

#[tokio::test]
async fn long_range_maps_render_overlays_and_visible_contacts_without_mutation() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 0, y: 3 },
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 30, None)),
    )
    .unwrap();
    let before = world.btech.clone();
    let terrain = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Terrain,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&terrain.text).lines().count(), 25);
    assert_eq!(
        text::plain(&terrain.text).lines().nth(2).unwrap(),
        "    012"
    );
    assert_eq!(
        text::plain(&terrain.text)
            .lines()
            .nth(10)
            .unwrap()
            .chars()
            .nth(4)
            .unwrap(),
        Terrain::Fire.symbol()
    );
    let units = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Units,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&units.text).lines().nth(5).unwrap(), "  1  * ");
    assert_eq!(text::plain(&units.text).lines().nth(7).unwrap(), "  2  b ");
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "lrsmap M"),
        units.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.lrsmap({},1,'M')", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&units).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_long_range_map(
            &restored,
            source,
            ObjectId(1),
            BattleLongRangeMode::Units,
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        units
    );
    let mut signature = world.btech.constructed_units()[&target].signature();
    signature.team = 2;
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    let enemy = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Units,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&enemy.text).lines().nth(7).unwrap(), "  2  B ");
}

/// Stacks use visible viewport membership ordering and do not give self a special priority.
#[tokio::test]
async fn long_range_stacked_markers_share_native_lua_and_restart_order() {
    for (target_first, earlier_inside, earlier_acquired, expected) in [
        (false, true, true, 'b'),
        (false, true, false, '*'),
        (false, false, true, '*'),
        (true, true, false, 'b'),
    ] {
        let (_dir, config, mut world, map, source, target) = fixture().await;
        stop_battle_unit(
            &mut world,
            source,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        place_battle_unit(&mut world, source, map, 1, 20).unwrap();
        assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        place_battle_unit(&mut world, target, map, 1, 20).unwrap();
        let earlier = world.create(&config, "Earlier hex".into(), Kind::Thing);
        world.objects.get_mut(&earlier).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            earlier,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, earlier, support::FIXTURE_DICE_SEED);
        place_battle_unit(
            &mut world,
            earlier,
            map,
            0,
            if earlier_inside { 19 } else { 0 },
        )
        .unwrap();
        acquire(&mut world, source, target);
        if earlier_acquired {
            acquire(&mut world, source, earlier);
        }
        if target_first {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][source.0.to_string()]["map_slot"] = 1.into();
            state["constructed"][target.0.to_string()]["map_slot"] = 0.into();
            world.btech = serde_json::from_value(state).unwrap();
        }
        let before = world.btech.clone();
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        for (selector, mode) in [
            ("M", BattleLongRangeMode::Units),
            ("S", BattleLongRangeMode::VisibleUnits),
        ] {
            let report = battle_long_range_map(
                &world,
                source,
                ObjectId(1),
                mode,
                "",
                BattleViewDimensions::default(),
            )
            .unwrap();
            assert_eq!(
                text::plain(&report.text).lines().nth(13).unwrap(),
                format!(" 20  {expected} ")
            );
            assert_eq!(
                support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("lrsmap {selector}")
                ),
                report.text
            );
            let lua: mlua::Table = scripts
                .eval_callback(&format!(
                    "return btech.unit.lrsmap({},1,'{selector}')",
                    source.0
                ))
                .unwrap();
            assert_eq!(
                serde_json::to_value(lua).unwrap(),
                serde_json::to_value(&report).unwrap()
            );
            assert!(scripts.drain_outbox().is_empty());
            assert_eq!(scripts.world().btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                battle_long_range_map(
                    &restored,
                    source,
                    ObjectId(1),
                    mode,
                    "",
                    BattleViewDimensions::default()
                )
                .unwrap(),
                report
            );
        }
        assert_eq!(world.btech, before);
    }
}

/// Long-range maps mask unreachable terrain and unacquired units; observers see projected centers.
#[tokio::test]
async fn long_range_maps_mask_dark_terrain_and_unacquired_units() {
    let (_dir, _config, mut world, map, source, _) = fixture().await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][source.0.to_string()]["contacts"] = serde_json::json!({});
    state["maps"][map.0.to_string()]["flags"] = 32.into();
    world.btech = serde_json::from_value(state).unwrap();
    blind(&mut world, map);
    let units = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Units,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!text::plain(&units.text).contains('b') && !text::plain(&units.text).contains('B'));
    assert!(units.text.contains('?') && units.text.contains('*'));
    let elevation = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Elevation,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(elevation.text.contains('?'));
    set_battle_observer(&mut world, source, true).unwrap();
    let far = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Terrain,
        "180 100",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(far.viewport.origin.y, 49);
    assert_eq!(far.text.matches('?').count(), 33);
    assert!(
        battle_long_range_map(
            &world,
            source,
            ObjectId(2),
            BattleLongRangeMode::Terrain,
            "",
            BattleViewDimensions::default()
        )
        .is_err()
    );
}

#[tokio::test]
async fn long_range_elevation_rows_preserve_zero_space_and_water_depth() {
    let (_dir, config, mut world, _map, source, _) = fixture().await;
    let map = world.create(&config, "Elevation display".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "elevation.map",
        BattleMapAsset::from_cells("3 2\n.0#3~2\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(&mut world, source, map, 1, 1).unwrap();
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let report = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Elevation,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!((report.viewport.width, report.viewport.height), (3, 2));
    assert_eq!(report.text.lines().nth(3).unwrap(), "  0  3 ");
    assert_eq!(report.text.lines().nth(4).unwrap(), "      2 0  ");
    assert!(!report.text.contains('~'));
}

#[tokio::test]
async fn long_range_colors_follow_ansi_and_keep_labels_outside_styles() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 0, y: 3 },
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 30, None)),
    )
    .unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Ansi);
    let plain = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Units,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!plain.text.contains("[fg=") && !plain.text.contains("[bold]"));
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Ansi);
    let colored = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Units,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&colored.text), plain.text);
    assert!(colored.text.contains("[bold]*") && colored.text.contains("[fg=yellow bold]b"));
    assert!(colored.text.contains("[fg=red bold]&"));
    assert_eq!(
        colored
            .text
            .lines()
            .nth(10)
            .unwrap()
            .rsplit("[reset]")
            .next()
            .unwrap()
            .trim(),
        "3"
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "lrsmap M"),
        colored.text
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.lrsmap({},1,'M').text",
            source.0
        ))
        .unwrap();
    assert_eq!(lua, colored.text);
    let ordinary = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Elevation,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!ordinary.text.contains("[fg="));
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Ansi);
    let elevation = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::ColoredElevation,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(elevation.text.contains("[fg=red bold]0"));
    assert_eq!(
        text::plain(&elevation.text).lines().nth(3).unwrap(),
        "  0  0 "
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "lrsmap C"),
        elevation.text
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_long_range_map(
            &restored,
            source,
            ObjectId(1),
            BattleLongRangeMode::ColoredElevation,
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        elevation
    );
}

/// Explicit visible-only long-range modes mask unreachable terrain in native, Lua and restart.
#[tokio::test]
async fn explicit_long_range_visibility_modes_filter_ordinary_maps() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    blind(&mut world, map);
    let before = world.btech.clone();
    let terrain = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::Terrain,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!terrain.text.contains('?'));
    let limited = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::VisibleTerrain,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(limited.text.contains('?'));
    assert!(limited.text.contains("[fg=blue]"));
    assert!(!limited.text.contains('X'));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    for (code, mode) in [
        ("L", BattleLongRangeMode::VisibleTerrain),
        ("H", BattleLongRangeMode::VisibleElevation),
        ("S", BattleLongRangeMode::VisibleUnits),
    ] {
        let expected = battle_long_range_map(
            &world,
            source,
            ObjectId(1),
            mode,
            "",
            BattleViewDimensions::default(),
        )
        .unwrap();
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, &format!("lrsmap {code}")),
            expected.text
        );
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.lrsmap({},1,'{code}').text",
                source.0
            ))
            .unwrap();
        assert_eq!(lua, expected.text);
        let plain = text::plain(&expected.text);
        assert!(!plain.contains('b') && !plain.contains('B'));
        assert_eq!(plain.contains('*'), code == "S");
    }
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_long_range_map(
            &restored,
            source,
            ObjectId(1),
            BattleLongRangeMode::VisibleTerrain,
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        limited
    );
    unblind(&mut world, map);
    let clear = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::VisibleTerrain,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(clear.text, terrain.text);
}

/// Terrain fires and inferno burns light hexes at night so sight reaches them beyond visibility.
#[tokio::test]
async fn terrain_fire_and_inferno_illumination_follow_live_sources_without_acquisition() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 3).unwrap();
    // Without the sensor band, only sight (and therefore illumination) reaches past three hexes.
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    let coordinate = BattleHexCoordinate { x: 1, y: 7 };
    let fire = BattleHexCoordinate { x: 1, y: 8 };
    assert!(!battle_hex_visible(&world, source, coordinate).unwrap());
    assert!(!battle_hex_illuminated(&world, map, coordinate).unwrap());
    set_map_decoration(
        &mut world,
        map,
        fire,
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 30, None)),
    )
    .unwrap();
    let before = world.btech.clone();
    assert!(battle_hex_illuminated(&world, map, fire).unwrap());
    assert!(battle_hex_illuminated(&world, map, coordinate).unwrap());
    assert!(!battle_hex_illuminated(&world, map, BattleHexCoordinate { x: 1, y: 6 }).unwrap());
    assert!(battle_hex_visible(&world, source, coordinate).unwrap());
    let rendered = battle_long_range_map(
        &world,
        source,
        ObjectId(1),
        BattleLongRangeMode::VisibleTerrain,
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(
        text::plain(&rendered.text)
            .lines()
            .nth(17)
            .unwrap()
            .chars()
            .nth(5)
            .unwrap(),
        ' '
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(battle_hex_illuminated(&restored, map, coordinate).unwrap());
    assert_eq!(
        battle_long_range_map(
            &restored,
            source,
            ObjectId(1),
            BattleLongRangeMode::VisibleTerrain,
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        rendered
    );
    set_map_decoration(&mut world, map, fire, None).unwrap();
    assert!(!battle_hex_visible(&world, source, coordinate).unwrap());
    apply_inferno_burn(&mut world, target, 1).unwrap();
    assert!(battle_hex_illuminated(&world, map, BattleHexCoordinate { x: 1, y: 3 }).unwrap());
    let _ = advance_inferno_burns(&mut world);
    assert!(!battle_hex_illuminated(&world, map, BattleHexCoordinate { x: 1, y: 3 }).unwrap());
    assert!(battle_hex_illuminated(&world, map, BattleHexCoordinate { x: -1, y: 0 }).is_err());
}

#[tokio::test]
async fn tactical_maps_render_contacts_and_underlying_terrain_with_native_lua_parity() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 0, y: 3 },
        Some(BattleDecoration::new(BattleDecorationKind::Fire, 30, None)),
    )
    .unwrap();
    let before = world.btech.clone();
    let report = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let plain = text::plain(&report.text);
    assert_eq!(plain.lines().count(), 32);
    assert_eq!(plain.lines().nth(2).unwrap(), "     0  1  2  ");
    assert_eq!(&plain.lines().nth(5).unwrap()[8..10], "**");
    assert_eq!(&plain.lines().nth(7).unwrap()[8..10], "ab");
    // Fire fills the top of its hex; the grassland beneath keeps the bottom.
    assert_eq!(&plain.lines().nth(10).unwrap()[5..7], "&&");
    assert_eq!(&plain.lines().nth(11).unwrap()[5..7], "__");
    assert!(
        battle_tactical_map(
            &world,
            source,
            ObjectId(1),
            "u",
            BattleViewDimensions::default(),
        )
        .is_err()
    );
    assert!(report.text.contains("[bold]**[reset]"));
    assert!(report.text.contains("[fg=yellow bold]ab[reset]"));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "tactical"),
        report.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.tactical({},1,'')", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_tactical_map(
            &restored,
            source,
            ObjectId(1),
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        report
    );
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Ansi);
    let monochrome = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&monochrome.text), plain);
    assert!(!monochrome.text.contains("[fg="));
    let mut signature = world.btech.constructed_units()[&target].signature();
    signature.team = 2;
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    let enemy = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(
        &text::plain(&enemy.text).lines().nth(7).unwrap()[8..10],
        "AB"
    );
}

/// Tactical maps mask hexes no channel reaches and share display admission with other views.
#[tokio::test]
async fn tactical_maps_mask_unseen_hexes_and_reuse_display_admission() {
    let (_dir, _config, mut world, map, source, _target) = fixture().await;
    blind(&mut world, map);
    let before = world.btech.clone();
    let ordinary = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let limited = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "L",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!ordinary.text.contains('?'));
    let plain = text::plain(&limited.text);
    assert!(plain.contains("??"));
    assert!(plain.contains("**"));
    assert!(!plain.contains("ab"));
    assert_eq!(world.btech, before);
    for args in ["Z", "L 0 NaN", "0 21", "ab"] {
        assert!(
            battle_tactical_map(
                &world,
                source,
                ObjectId(1),
                args,
                BattleViewDimensions::default()
            )
            .is_err(),
            "{args}"
        );
    }
    assert!(
        battle_tactical_map(
            &world,
            source,
            ObjectId(3),
            "",
            BattleViewDimensions::default()
        )
        .is_err()
    );
    let mut value = serde_json::to_value(&world.btech).unwrap();
    value["maps"][map.0.to_string()]["flags"] = serde_json::json!(32);
    world.btech = serde_json::from_value(value).unwrap();
    assert_eq!(
        battle_tactical_map(
            &world,
            source,
            ObjectId(1),
            "",
            BattleViewDimensions::default()
        )
        .unwrap(),
        limited
    );
    set_battle_observer(&mut world, source, true).unwrap();
    let projected = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "180 100",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(projected.viewport.origin.y, 46);
    assert!(!text::plain(&projected.text).contains("**"));
}

#[tokio::test]
async fn tactical_clipping_preserves_global_hex_parity_and_elevation() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    let map = world.create(&config, "Wide tactical field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "wide.map",
        BattleMapAsset::from_cells("8 3\n.0.0.0#3~2.0.0.0\n.0.0.0.0.0.0.0.0\n.0.0.0.0.0.0.0.0\n")
            .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(&mut world, source, map, 5, 1).unwrap();
    place_battle_unit(&mut world, target, map, 6, 1).unwrap();
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    acquire(&mut world, source, target);
    let report = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "",
        BattleViewDimensions {
            tactical_width: 5,
            tactical_height: 5,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(report.viewport.origin, BattleHexCoordinate { x: 3, y: 0 });
    assert_eq!((report.viewport.width, report.viewport.height), (5, 3));
    let plain = text::plain(&report.text);
    let lines: Vec<_> = plain.lines().collect();
    assert_eq!(lines.len(), 10);
    assert_eq!(&lines[3][5..7], "##");
    assert_eq!(&lines[4][5..7], "#3");
    assert_eq!(&lines[4][8..10], "~~");
    assert_eq!(&lines[5][8..10], "~2");
    assert_eq!(&lines[5][11..13], "**");
    assert_eq!(&lines[6][14..16], "ab");
    let mines = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "M",
        BattleViewDimensions {
            tactical_width: 5,
            tactical_height: 5,
            ..Default::default()
        },
    )
    .unwrap();
    let plain = text::plain(&mines.text);
    let lines: Vec<_> = plain.lines().collect();
    assert_eq!(&lines[3][5..7], "#3");
    assert_eq!(&lines[4][5..7], "  ");
    assert_eq!(&lines[4][8..10], "~2");
    assert_eq!(&lines[5][8..10], "  ");
    assert_eq!(&lines[6][14..16], "ab");
}

#[tokio::test]
async fn tactical_cliffs_use_signed_depth_thresholds_and_share_native_lua_output() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    let map = world.create(&config, "Cliff field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "cliffs.map",
        BattleMapAsset::from_cells("3 3\n.0.3.0\n.0.0~2\n.0-3.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(&mut world, source, map, 1, 1).unwrap();
    place_battle_unit(&mut world, target, map, 2, 1).unwrap();
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    acquire(&mut world, source, target);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Ansi);
    // Decoration must not change the submerged cliff height.
    set_map_decoration(
        &mut world,
        map,
        BattleHexCoordinate { x: 1, y: 2 },
        Some(BattleDecoration::new(BattleDecorationKind::Smoke, 30, None)),
    )
    .unwrap();
    let before = world.btech.clone();
    let mech = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "C",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let tank = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "T",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let mech_plain = text::plain(&mech.text);
    let lines: Vec<_> = mech_plain.lines().collect();
    assert_eq!(&lines[3][8..10], " 3");
    assert_eq!(&lines[4][7..11], "|,,!");
    assert_eq!(&lines[5][8..10], "**");
    assert_eq!(&lines[6][8..11], ",,/");
    assert_eq!(
        &text::plain(&tank.text).lines().nth(6).unwrap()[8..11],
        ",,!"
    );
    assert!(!mech_plain.contains("ab"));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    for (args, expected) in [("C", &mech), ("T", &tank)] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("tactical {args}")
            ),
            expected.text
        );
        let lua: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.tactical({},1,'{args}')",
                source.0
            ))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_tactical_map(
            &restored,
            source,
            ObjectId(1),
            "C",
            BattleViewDimensions::default()
        )
        .unwrap(),
        mech
    );
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Ansi);
    let colored = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "T",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(colored.text.contains("[fg=red bold]"));
    assert_eq!(
        &text::plain(&colored.text).lines().nth(6).unwrap()[8..11],
        "__/"
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 32.into();
    world.btech = serde_json::from_value(state).unwrap();
    for flag in ["C", "T"] {
        let error = battle_tactical_map(
            &world,
            source,
            ObjectId(1),
            flag,
            BattleViewDimensions::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("You can't see that much here!"));
    }
}

#[tokio::test]
async fn landing_overlays_honor_saved_team_exclusions_and_terrain() {
    let (_dir, config, mut world, map, source, _target) = fixture().await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    let query = |world: &World, coordinate, team| {
        world.btech.maps()[&map]
            .landing_suitability(coordinate, team)
            .unwrap()
    };
    assert_eq!(query(&world, center, 0), BattleLandingSuitability::Ready);
    assert_eq!(
        query(&world, BattleHexCoordinate { x: 0, y: 1 }, 0),
        BattleLandingSuitability::UnevenGround
    );
    assert!(
        world.btech.maps()[&map]
            .landing_suitability(BattleHexCoordinate { x: -1, y: 0 }, 0)
            .is_err()
    );
    let zone = BattleLandingExclusion {
        coordinate: center,
        radius: 1,
        exempt_team: 2,
        owner: ObjectId(1),
        data_short: 0,
    };
    set_battle_landing_exclusion(&mut world, map, 7, Some(zone)).unwrap();
    assert_eq!(query(&world, center, 0), BattleLandingSuitability::Blocked);
    assert_eq!(query(&world, center, 2), BattleLandingSuitability::Ready);
    assert_eq!(
        query(&world, BattleHexCoordinate { x: 1, y: 2 }, 0),
        BattleLandingSuitability::Blocked
    );
    assert_eq!(
        query(&world, BattleHexCoordinate { x: 1, y: 3 }, 0),
        BattleLandingSuitability::Ready
    );
    let before = world.btech.clone();
    assert!(
        set_battle_landing_exclusion(
            &mut world,
            map,
            8,
            Some(BattleLandingExclusion {
                coordinate: BattleHexCoordinate { x: 3, y: 1 },
                ..zone
            })
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let report = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "B",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let plain = text::plain(&report.text);
    assert_eq!(&plain.lines().nth(5).unwrap()[8..10], "**");
    assert_eq!(&plain.lines().nth(6).unwrap()[8..9], "X");
    assert!(!plain.contains("ab"));
    assert!(report.text.contains("[fg=green bold]O"));
    assert!(report.text.contains("[fg=red bold]X"));
    let mut monochrome = world.clone();
    monochrome
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .remove(Flag::Ansi);
    let monochrome_report = battle_tactical_map(
        &monochrome,
        source,
        ObjectId(1),
        "B",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(text::plain(&monochrome_report.text), plain);
    assert!(!monochrome_report.text.contains("[fg="));

    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "tactical B"),
        report.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.tactical({},1,'B')", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech.maps()[&map].landing_exclusions()[&7], zone);
    assert_eq!(
        battle_tactical_map(
            &restored,
            source,
            ObjectId(1),
            "B",
            BattleViewDimensions::default()
        )
        .unwrap(),
        report
    );
    let mut signature = world.btech.constructed_units()[&source].signature();
    signature.team = 2;
    set_battle_unit_signature(&mut world, source, signature).unwrap();
    let exempt = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "B",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert_eq!(
        &text::plain(&exempt.text).lines().nth(6).unwrap()[8..9],
        "O"
    );
    set_battle_landing_exclusion(&mut world, map, 7, None).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(restored.btech.maps()[&map].landing_exclusions().is_empty());
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 32.into();
    world.btech = serde_json::from_value(state).unwrap();
    assert!(
        battle_tactical_map(
            &world,
            source,
            ObjectId(1),
            "B",
            BattleViewDimensions::default()
        )
        .unwrap_err()
        .to_string()
        .contains("You can't see that much here!")
    );
}

#[tokio::test]
async fn landing_suitability_checks_full_hex_neighborhood_terrain_and_fire() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let center = BattleHexCoordinate { x: 1, y: 1 };
    for (tiles, flags, expected) in [
        (
            "~0~0~0\n~0/0~0\n~0~0~0\n",
            0,
            BattleLandingSuitability::ImproperTerrain,
        ),
        (
            "~0~0~0\n~0#0~0\n~0~0~0\n",
            0,
            BattleLandingSuitability::Ready,
        ),
        (
            ".0.1.0\n.0#0.0\n.0.0.0\n",
            0,
            BattleLandingSuitability::UnevenGround,
        ),
        (
            ".0.0.0\n.0-0.0\n.0.0.0\n",
            0,
            BattleLandingSuitability::ImproperTerrain,
        ),
    ] {
        let map = world.create(&config, "Landing terrain".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "landing.map",
            BattleMapAsset::from_cells(&format!("3 3\n{tiles}{flags}: 100 20\n")).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        assert_eq!(
            world.btech.maps()[&map]
                .landing_suitability(center, 0)
                .unwrap(),
            expected
        );
        // Nothing lands in fire or smoke, whatever the ground beneath.
        for kind in [BattleDecorationKind::Fire, BattleDecorationKind::Smoke] {
            set_map_decoration(
                &mut world,
                map,
                center,
                Some(BattleDecoration::new(kind, 30, None)),
            )
            .unwrap();
            assert_eq!(
                world.btech.maps()[&map]
                    .landing_suitability(center, 0)
                    .unwrap(),
                BattleLandingSuitability::ImproperTerrain
            );
        }
    }
}

/// Tactical mine overlays follow trigger fields and hex visibility without recognition rolls.
#[tokio::test]
async fn tactical_mines_filter_trigger_fields_and_visibility_without_recognition() {
    let (_dir, config, mut world, map, source, _target) = fixture().await;
    let coordinate = BattleHexCoordinate { x: 1, y: 2 };
    let mine = BattleMinefield {
        coordinate,
        kind: BattleMineKind::Command,
        strength: 19,
        extra: 4321,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 1, Some(mine)).unwrap();
    let before = world.btech.clone();
    let report = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "M",
        BattleViewDimensions::default(),
    )
    .unwrap();
    let plain = text::plain(&report.text);
    assert_eq!(&plain.lines().nth(7).unwrap()[8..10], "ab");
    assert_eq!(&plain.lines().nth(8).unwrap()[8..10], "<>");
    assert_eq!(&plain.lines().nth(5).unwrap()[8..10], "**");
    assert!(!plain.contains("4321"));
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "tactical M"),
        report.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.tactical({},1,'M')", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_tactical_map(
            &restored,
            source,
            ObjectId(1),
            "M",
            BattleViewDimensions::default()
        )
        .unwrap(),
        report
    );
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            kind: BattleMineKind::Trigger,
            ..mine
        }),
    )
    .unwrap();
    let trigger = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "M",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!text::plain(&trigger.text).contains("<>"));
    set_minefield(&mut world, map, 0, None).unwrap();
    blind(&mut world, map);
    let hidden = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "M",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(!text::plain(&hidden.text).contains("<>"));
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 32.into();
    world.btech = serde_json::from_value(state).unwrap();
    let dark = battle_tactical_map(
        &world,
        source,
        ObjectId(1),
        "M",
        BattleViewDimensions::default(),
    )
    .unwrap();
    assert!(text::plain(&dark.text).contains('?'));
    assert!(!text::plain(&dark.text).contains("<>"));
}

#[tokio::test]
async fn findcenter_measures_continuous_position_without_sensor_hardware() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    let centered = find_battle_hex_center(&world, source, ObjectId(1)).unwrap();
    assert_eq!(centered.range, 0.0);
    assert_eq!(centered.bearing, 180);
    assert_eq!(
        centered.text,
        "Current hex: (1,1,0)\tRange to center: 0.00\tBearing to center: 180"
    );
    let center = BattleHexCoordinate { x: 1, y: 1 }.center();
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["motion"]["point"] =
                serde_json::to_value(center.project(270.0, 0.2).unwrap()).unwrap();
            record["lost_criticals"] =
                serde_json::json!([{"section":"Head","slot":1},{"section":"Head","slot":4}]);
        })
        .unwrap();
    let before = world.btech.clone();
    let report = find_battle_hex_center(&world, source, ObjectId(1)).unwrap();
    assert!((report.range - 0.2).abs() < 1e-10);
    assert_eq!(report.bearing, 90);
    assert!(report.text.contains("Range to center: 0.20"));
    assert_eq!(
        world.btech.constructed_units()[&source]
            .sensor_ranges()
            .tactical,
        0
    );
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "findcenter"),
        report.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.findcenter({},1)", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        find_battle_hex_center(&restored, source, ObjectId(1)).unwrap(),
        report
    );
    assert!(find_battle_hex_center(&world, source, ObjectId(3)).is_err());
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    assert!(find_battle_hex_center(&world, source, ObjectId(1)).is_err());
}

#[tokio::test]
async fn navigation_combines_local_map_continuous_plot_and_readouts_without_mutation() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    place_battle_unit(&mut world, target, map, 1, 1).unwrap();
    acquire(&mut world, source, target);
    let center = BattleHexCoordinate { x: 1, y: 1 };
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["motion"]["point"] =
                serde_json::to_value(center.center().project(0.0, 0.2).unwrap()).unwrap();
        })
        .unwrap();
    let before = world.btech.clone();
    let report = battle_navigate(&world, source, ObjectId(1), "").unwrap();
    let plain = text::plain(&report.text);
    let lines: Vec<_> = plain.lines().collect();
    assert_eq!(lines.len(), 13);
    assert_eq!(lines[4].chars().nth(14), Some('x'));
    assert_eq!(lines[6].chars().nth(14), Some('*'));
    assert!(lines[2].contains("Location:   1,   1,   0"));
    assert!(lines[3].contains("Grassland"));
    assert!(lines[6].contains("Speed:"));
    assert!(lines[8].contains("Heading:"));
    assert_eq!(report.center, center);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "navigate"),
        report.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.navigate({},1)", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_navigate(&restored, source, ObjectId(1), "").unwrap(),
        report
    );
    assert_eq!(
        battle_navigate(&world, source, ObjectId(1), "ab").unwrap(),
        report
    );
    let mut signature = world.btech.constructed_units()[&target].signature();
    signature.team = 2;
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    assert_eq!(
        text::plain(
            &battle_navigate(&world, source, ObjectId(1), "")
                .unwrap()
                .text
        )
        .lines()
        .nth(4)
        .unwrap()
        .chars()
        .nth(14),
        Some('X')
    );
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 32.into();
    state["constructed"][source.0.to_string()]["lost_criticals"] =
        serde_json::json!([{"section":"Head","slot":1},{"section":"Head","slot":4}]);
    world.btech = serde_json::from_value(state).unwrap();
    let dark = battle_navigate(&world, source, ObjectId(1), "").unwrap();
    let plain = text::plain(&dark.text);
    assert!(plain.contains('?'));
    assert_ne!(plain.lines().nth(4).unwrap().chars().nth(14), Some('X'));
    assert!(battle_navigate(&world, source, ObjectId(3), "").is_err());
    assert!(battle_navigate(&world, source, ObjectId(1), "0 1").is_err());
    set_battle_observer(&mut world, source, true).unwrap();
    let remote = battle_navigate(&world, source, ObjectId(1), "180 100").unwrap();
    assert_eq!(remote.center, BattleHexCoordinate { x: 1, y: 101 });
    assert!(!text::plain(&remote.text).contains('*'));
}

#[tokio::test]
async fn navigation_keeps_even_center_on_single_hex_maps_with_off_map_surroundings() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    let map = world.create(&config, "One navigation hex".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "one.map",
        BattleMapAsset::from_cells("1 1\n#3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(&mut world, source, map, 0, 0).unwrap();
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let report = battle_navigate(&world, source, ObjectId(1), "").unwrap();
    assert_eq!(report.center, BattleHexCoordinate { x: 0, y: 0 });
    let plain = text::plain(&report.text);
    assert!(plain.contains("Location:   0,   0,   3"));
    assert!(plain.contains("Road"));
    let map: String = plain
        .lines()
        .take(11)
        .filter_map(|line| line.get(57..))
        .collect();
    assert_eq!(map.matches('#').count(), 1);
    assert_eq!(map.matches("**").count(), 1);
    assert!(map.contains("#3"));
}

#[tokio::test]
async fn saved_view_dimensions_drive_native_lua_maps_and_preserve_other_configuration() {
    use sqlx::{Connection, Row};
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    assert_eq!(
        battle_view_dimensions(&world, ObjectId(1)).unwrap(),
        BattleViewDimensions::default()
    );
    let before = world.btech.clone();
    assert!(
        set_battle_view_dimensions(
            &mut world,
            ObjectId(1),
            BattleViewDimensions {
                tactical_width: 4,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        set_battle_view_dimensions(&mut world, source, BattleViewDimensions::default()).is_err()
    );
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let _: mlua::Table = scripts
        .eval_callback(
            "return btech.player.view_dimensions(1,{tactical_height=6,long_range_height=40})",
        )
        .unwrap();
    let dimensions = BattleViewDimensions {
        tactical_height: 6,
        long_range_height: 40,
        ..Default::default()
    };
    assert_eq!(
        battle_view_dimensions(&scripts.world(), ObjectId(1)).unwrap(),
        dimensions
    );
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(
                "btech.player.view_dimensions(1,{tactical_width=10}); error('abort')"
            )
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(
        scripts
            .eval_callback::<mlua::Table>(
                "return btech.player.view_dimensions(1,{long_range_height=9})"
            )
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let tactical =
        battle_tactical_map(&scripts.world(), source, ObjectId(1), "", dimensions).unwrap();
    assert_eq!(tactical.viewport.height, 6);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "tactical"),
        tactical.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.tactical({},1)", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&tactical).unwrap()
    );
    let lrs = battle_long_range_map(
        &scripts.world(),
        source,
        ObjectId(1),
        BattleLongRangeMode::Terrain,
        "",
        dimensions,
    )
    .unwrap();
    assert_eq!(lrs.viewport.height, 41);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "lrsmap T"),
        lrs.text
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.lrsmap({},1,'T')", source.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&lrs).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    let height: u16 = scripts
        .eval_callback(&format!(
            "return btech.unit.viewport({},1,'tactical').height",
            source.0
        ))
        .unwrap();
    assert_eq!(height, 6);
    assert!(scripts.drain_outbox().is_empty());
    let mut world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_view_dimensions(&restored, ObjectId(1)).unwrap(),
        dimensions
    );
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_player_configuration SET include_shutdown=0,has_loadout=1,right_weapon='test weapon',technician_available_at=12345 WHERE player_dbref=1").execute(&mut sql).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    set_battle_view_dimensions(&mut world, ObjectId(1), BattleViewDimensions::default()).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let row = sqlx::query("SELECT include_shutdown,has_loadout,right_weapon,technician_available_at FROM btech_player_configuration WHERE player_dbref=1").fetch_one(&mut sql).await.unwrap();
    assert_eq!(row.get::<i64, _>("include_shutdown"), 0);
    assert_eq!(row.get::<i64, _>("has_loadout"), 1);
    assert_eq!(row.get::<String, _>("right_weapon"), "test weapon");
    assert_eq!(row.get::<i64, _>("technician_available_at"), 12345);
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_view_dimensions(&restored, ObjectId(1)).unwrap(),
        BattleViewDimensions::default()
    );
    sqlx::query("UPDATE btech_player_configuration SET tactical_width=4 WHERE player_dbref=1")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn mapdisplay_edits_only_its_player_and_survives_restart() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    release_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ObjectId(config.home()));
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "mapdisplay"),
        "Map display: tactical 21 wide by 14 high; long-range height 11."
    );
    assert_eq!(scripts.world().btech, before);
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "mapdisplay 30 20 40"),
        "Map display: tactical 30 wide by 20 high; long-range height 40."
    );
    let dimensions = BattleViewDimensions {
        tactical_width: 30,
        tactical_height: 20,
        long_range_height: 40,
    };
    assert_eq!(
        battle_view_dimensions(&scripts.world(), ObjectId(1)).unwrap(),
        dimensions
    );
    // Every reference boundary is accepted before hardware/map clipping.
    for width in [5, 40] {
        for height in [5, 24] {
            for lrs in [10, 40] {
                let reply = support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("mapdisplay {width} {height} {lrs}"),
                );
                assert_eq!(
                    reply,
                    format!(
                        "Map display: tactical {width} wide by {height} high; long-range height {lrs}."
                    )
                );
                assert_eq!(
                    battle_view_dimensions(&scripts.world(), ObjectId(1)).unwrap(),
                    BattleViewDimensions {
                        tactical_width: width,
                        tactical_height: height,
                        long_range_height: lrs,
                    }
                );
            }
        }
    }
    support::run_text(&scripts, &config, ObjectId(1), 1, "mapdisplay 30 20 40");
    let before = scripts.world().btech.clone();
    for args in [
        "4 20 40",
        "41 20 40",
        "30 4 40",
        "30 25 40",
        "30 20 9",
        "30 20 41",
        "-1 20 40",
        "30 20",
        "#3 30 20 40",
        "1.5 20 40",
    ] {
        let reply = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("mapdisplay {args}"),
        );
        assert!(!reply.starts_with("Map display:"), "{args}");
        assert_eq!(scripts.world().btech, before);
    }
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "mapdisplay/reset")
            .contains("takes no switches")
    );
    assert_eq!(scripts.world().btech, before);
    assert_eq!(
        scripts.world().btech.constructed_units()[&source],
        before.constructed_units()[&source]
    );
    let world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert!(
        support::run_text(&restarted, &config, ObjectId(1), 1, "mapdisplay")
            .contains("30 wide by 20 high")
    );
    assert_eq!(
        support::run_text(&restarted, &config, ObjectId(1), 1, "mapdisplay RESET"),
        "Map display: tactical 21 wide by 14 high; long-range height 11."
    );
    let lua: mlua::Table = restarted
        .eval_callback("return btech.player.view_dimensions(1)")
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(BattleViewDimensions::default()).unwrap()
    );
    assert!(restarted.drain_outbox().is_empty());
}

/// Saved contact preferences filter lists but never show targets that are no longer perceived.
#[tokio::test]
async fn contact_preferences_filter_lists_without_bypassing_acquisition() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    let hidden = BattleContactPreferences {
        include_allies: false,
        include_enemies: false,
        include_shutdown: false,
        include_dead: false,
        include_target: false,
        ..Default::default()
    };
    set_battle_contact_preferences(&mut world, ObjectId(1), hidden).unwrap();
    assert!(
        filtered_battle_contacts(&world, source, hidden)
            .unwrap()
            .is_empty()
    );
    assert_eq!(visible_battle_contacts(&world, source).unwrap().len(), 1);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +"),
        "Line of Sight Contacts:\r\nEnd Contact List"
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts")
            .lines()
            .filter(|line| line.contains(" x:"))
            .count()
            == 1
    );
    let before = scripts.world().btech.clone();
    let lua: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({},btech.player.contact_preferences(1))",
            source.0
        ))
        .unwrap();
    assert_eq!(lua.raw_len(), 0);
    assert!(
        scripts
            .eval_callback::<()>(
                "btech.player.contact_preferences(1,{include_allies=true}); error('abort')"
            )
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let mut signature = world.btech.constructed_units()[&target].signature();
    signature.team = 2;
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    let enemies = BattleContactPreferences {
        include_allies: false,
        include_target: false,
        ..Default::default()
    };
    assert_eq!(
        filtered_battle_contacts(&world, source, enemies)
            .unwrap()
            .len(),
        1
    );
    assert!(
        filtered_battle_contacts(
            &world,
            source,
            BattleContactPreferences {
                include_enemies: false,
                ..enemies
            }
        )
        .unwrap()
        .is_empty()
    );
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let selected = BattleContactPreferences {
        include_target: true,
        ..hidden
    };
    assert_eq!(
        filtered_battle_contacts(&world, source, selected)
            .unwrap()
            .len(),
        1
    );
    let no_shutdown = BattleContactPreferences {
        include_shutdown: false,
        include_target: false,
        ..Default::default()
    };
    assert!(
        filtered_battle_contacts(&world, source, no_shutdown)
            .unwrap()
            .is_empty()
    );
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["pilot_injuries"] = 6.into();
            record["pilot_killed"] = true.into();
        })
        .unwrap();
    assert!(world.btech.constructed_units()[&target].is_destroyed());
    let dead = BattleContactPreferences {
        include_dead: true,
        ..no_shutdown
    };
    assert_eq!(
        filtered_battle_contacts(&world, source, dead)
            .unwrap()
            .len(),
        1
    );
    assert!(
        filtered_battle_contacts(&world, source, no_shutdown)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        filtered_battle_contacts(&world, source, selected)
            .unwrap()
            .len(),
        1
    );
    set_battle_contact_preferences(&mut world, ObjectId(1), selected).unwrap();
    let dimensions = BattleViewDimensions {
        tactical_height: 6,
        ..Default::default()
    };
    set_battle_view_dimensions(&mut world, ObjectId(1), dimensions).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_contact_preferences(&restored, ObjectId(1)).unwrap(),
        selected
    );
    assert_eq!(
        battle_view_dimensions(&restored, ObjectId(1)).unwrap(),
        dimensions
    );
    blind(&mut world, map);
    assert!(
        filtered_battle_contacts(&world, source, selected)
            .unwrap()
            .is_empty()
    );
}

/// Contact option strings filter one listing without saving preferences, matching Lua filters.
#[tokio::test]
async fn contact_option_strings_are_transient_and_match_lua_filtering() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    for options in ["as", "!", "!e"] {
        assert!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("contacts {options}")
            )
            .lines()
            .filter(|line| line.contains(" x:"))
            .count()
                == 1
        );
        let count: usize = scripts.eval_callback(&format!("return #btech.unit.contacts({},btech.player.contact_options('{options}').preferences)", source.0)).unwrap();
        assert_eq!(count, 1);
    }
    for options in ["a", "e", "!s", "!a", "t"] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("contacts {options}")
            ),
            "Line of Sight Contacts:\r\nEnd Contact List"
        );
    }
    let unknown = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts asQ");
    assert!(unknown.starts_with("Ignoring Q as contact option."));
    assert!(unknown.lines().filter(|line| line.contains(" x:")).count() == 1);
    let lua: mlua::Table = scripts
        .eval_callback("return btech.player.contact_options('asQ')")
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(parse_battle_contact_options("asQ").unwrap()).unwrap()
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b"),
        "Line of Sight Contacts:\r\nEnd Contact List"
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let selected = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&selected, &config, ObjectId(1), 1, "contacts t")
            .lines()
            .filter(|line| line.contains(" x:"))
            .count()
            == 1
    );
    assert!(
        support::run_text(&selected, &config, ObjectId(1), 1, "contacts !asde")
            .lines()
            .filter(|line| line.contains(" x:"))
            .count()
            == 1
    );
    let mut saved = selected.world().clone();
    blind(&mut saved, map);
    assert!(
        filtered_battle_contacts(
            &saved,
            source,
            parse_battle_contact_options("t").unwrap().preferences
        )
        .unwrap()
        .is_empty()
    );
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_contact_preferences(&restored, ObjectId(1)).unwrap(),
        BattleContactPreferences::default()
    );
}

/// Building contacts identify concealed structures, roll back failed locks and need perception.
#[tokio::test]
async fn building_contacts_identify_concealed_structures_and_rollback_failed_locks() {
    let (_dir, config, mut world, map, source, _target) = fixture().await;
    let interior = scan_structure(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    let contacts = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].name, "Hangar");
    assert_eq!(
        (contacts[0].integrity, contacts[0].maximum_integrity),
        (31, 50)
    );
    assert_eq!(contacts[0].elevation, 1);
    assert_eq!(contacts[0].weapon_arc, BattleContactArc::Rear);
    assert!(contacts[0].identified);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b")
            .contains(&contacts[0].text())
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts bas")
            .lines()
            .filter(|line| line.contains(" x:"))
            .count()
            == 2
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!(
            "return btech.unit.building_contacts({},1)",
            source.0
        ))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&contacts).unwrap()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("_parents", parents)
        .unwrap();
    scripts.eval_callback::<()>(&format!("calls=0; _parents['default_room.lua'].locks={{identify_building=function(ctx) calls=calls+1; assert(ctx.object=={} and ctx.enactor==1 and ctx.subject==1 and ctx.cause=={} and ctx.silent); return false end}}", interior.0, source.0)).unwrap();
    let denied = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
    assert_eq!(denied[0].status, 'x');
    assert!(!denied[0].identified);
    let mut building = scripts.world().btech.maps()[&interior].building;
    for (flags, allowed, status) in [(1, true, 'C'), (1, false, 'X'), (8, true, 'X')] {
        building.flags = flags;
        set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
        scripts.eval_callback::<()>(&format!("_parents['default_room.lua'].locks.identify_building=function(ctx) calls=calls+1; return {allowed} end")).unwrap();
        assert_eq!(
            battle_building_contacts(&scripts, source, ObjectId(1)).unwrap()[0].status,
            status
        );
    }
    scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) calls=calls+1; return false end").unwrap();
    building.flags = 4;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    assert!(
        battle_building_contacts(&scripts, source, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    scripts.eval_callback::<()>("_parents['default_room.lua'].locks.identify_building=function(ctx) calls=calls+1; return true end").unwrap();
    let hidden = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
    assert!(hidden[0].hidden && hidden[0].identified);
    let calls: i64 = scripts.eval_callback("return calls").unwrap();
    building.flags = 16;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    assert!(
        battle_building_contacts(&scripts, source, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(scripts.eval_callback::<i64>("return calls").unwrap(), calls);
    building.flags = 0;
    set_building_state(&mut shared.borrow_mut(), interior, building).unwrap();
    scripts.eval_callback::<()>(&format!("_parents['default_room.lua'].locks.identify_building=function(ctx) btech.player.view_dimensions(1,{{tactical_height=6}}); btech.unit.scan_building({},1,1,2); error('lock failed') end", source.0)).unwrap();
    let before = scripts.world().btech.clone();
    assert!(battle_building_contacts(&scripts, source, ObjectId(1)).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    scripts
        .eval_callback::<()>(
            "_parents['default_room.lua'].locks.identify_building=function(ctx) return true end",
        )
        .unwrap();
    blind(&mut shared.borrow_mut(), map);
    assert!(
        battle_building_contacts(&scripts, source, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    unblind(&mut shared.borrow_mut(), map);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        battle_building_contacts(&restarted, source, ObjectId(1)).unwrap(),
        contacts
    );
    assert!(battle_building_contacts(&restarted, source, ObjectId(3)).is_err());
}

#[tokio::test]
async fn saved_building_preferences_control_contacts_and_survive_restart() {
    use sqlx::{Connection, Row};
    let (_dir, config, mut world, map, source, _target) = fixture().await;
    let _ = scan_structure(&mut world, &config, map);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(!support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +").contains("Hangar"));
    for (mode, includes, stored) in [
        ("include", true, 1),
        ("follow_brief", true, 0),
        ("exclude", false, 2),
    ] {
        let _: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.player.contact_preferences(1,{{buildings='{mode}'}})"
            ))
            .unwrap();
        let report = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts +");
        assert_eq!(report.contains("Hangar"), includes);
        // Explicit building requests are independent of saved policy.
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b").contains("Hangar")
        );
        let _: mlua::Table = scripts
            .eval_callback("return btech.player.view_dimensions(1,{tactical_height=6})")
            .unwrap();
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_contact_preferences(&restored, ObjectId(1)).unwrap(),
            battle_contact_preferences(&saved, ObjectId(1)).unwrap()
        );
        assert_eq!(
            battle_view_dimensions(&restored, ObjectId(1))
                .unwrap()
                .tactical_height,
            6
        );
        let restarted =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        assert_eq!(
            support::run_text(&restarted, &config, ObjectId(1), 1, "contacts +"),
            report
        );
        let mut sql = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
        )
        .await
        .unwrap();
        let row =
            sqlx::query("SELECT buildings FROM btech_player_configuration WHERE player_dbref=1")
                .fetch_one(&mut sql)
                .await
                .unwrap();
        assert_eq!(row.get::<i64, _>("buildings"), stored);
    }
    let before = scripts.world().btech.clone();
    for value in ["'unknown'", "1", "true"] {
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.player.contact_preferences(1,{{buildings={value}}})"
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    assert!(
        scripts
            .eval_callback::<()>(
                "btech.player.contact_preferences(1,{buildings='include'}); error('abort')"
            )
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    // A pure unit list does not run structure locks or acquire buildings.
    let count: usize = scripts
        .eval_callback(&format!(
            "return #btech.unit.contacts({},{{buildings='include'}})",
            source.0
        ))
        .unwrap();
    assert_eq!(count, 1);
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_player_configuration SET buildings=3 WHERE player_dbref=1")
        .execute(&mut sql)
        .await
        .unwrap();
    assert!(persistence::load(&config.database()).await.is_err());
}

#[tokio::test]
async fn brief_modes_are_unit_owned_and_control_buildings_and_notices() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    set_battle_autocon_shutdown(&mut world, source, ObjectId(1), true).unwrap();
    let _ = scan_structure(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(source);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        !battle_brief(&scripts, source, ObjectId(2), "")
            .unwrap()
            .changed
    );
    assert_eq!(
        battle_building_contacts(&scripts, source, ObjectId(2))
            .unwrap()
            .len(),
        1
    );
    let initial = scripts.world().btech.clone();
    let query = battle_brief(&scripts, source, ObjectId(1), "").unwrap();
    assert_eq!(query.settings, BattleBriefSettings::default());
    assert!(!query.changed);
    assert_eq!(scripts.world().btech, initial);
    assert!(scripts.drain_outbox().is_empty());
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "contacts").contains("Hangar"));
    let _: mlua::Table = scripts
        .eval_callback("return btech.player.contact_preferences(1,{buildings='follow_brief'})")
        .unwrap();
    for mode in 0..=3 {
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, &format!("brief c{mode}"))
                .contains("Contact brevity set")
        );
        for command in ["contacts", "contacts +", "contacts as"] {
            assert_eq!(
                support::run_text(&scripts, &config, ObjectId(1), 1, command).contains("Hangar"),
                mode == 1
            );
        }
        assert!(
            !support::run_text(&scripts, &config, ObjectId(1), 1, "contacts !").contains("Hangar")
        );
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b").contains("Hangar")
        );
        let parsed = parse_battle_contact_options_for_display("as", mode == 1).unwrap();
        let lua: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.player.contact_options('as',{})",
                mode == 1
            ))
            .unwrap();
        assert_eq!(
            serde_json::to_value(lua).unwrap(),
            serde_json::to_value(parsed).unwrap()
        );
    }
    let event = BattleContactEvent {
        experience_message: None,
        identified: true,
        observer: source,
        target,
        acquired: true,
        lock_lost: false,
    };
    for mode in 0..=6 {
        let _: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.unit.brief({},1,'A {mode}')",
                source.0
            ))
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&source]
                .brief_settings()
                .contacts,
            3
        );
        assert_eq!(
            event.notice(&scripts.world()).is_some(),
            !matches!(mode, 2 | 3 | 5 | 6)
        );
        let mut hostile = scripts.world().clone();
        let mut signature = hostile.btech.constructed_units()[&target].signature();
        signature.team = 99;
        set_battle_unit_signature(&mut hostile, target, signature).unwrap();
        assert_eq!(event.notice(&hostile).is_some(), mode != 6);
        let warning = BattleContactEvent {
            acquired: false,
            lock_lost: true,
            ..event.clone()
        }
        .notice(&scripts.world())
        .unwrap();
        assert!(warning.text.contains("lock has been lost"));
    }
    let _ = scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    for arguments in ["C", "C -1", "C 4", "A 7", "A 1.5", "C 1 extra", "Z 0"] {
        assert!(battle_brief(&scripts, source, ObjectId(1), arguments).is_err());
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    assert!(battle_brief(&scripts, source, ObjectId(3), "C 1").is_err());
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.brief({},1,'C 1'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.constructed_units()[&source].brief_settings(),
        BattleBriefSettings {
            contacts: 3,
            automatic: 6
        }
    );
    assert_eq!(
        restored.btech.constructed_units()[&target].brief_settings(),
        BattleBriefSettings::default()
    );
    let mut encoded = serde_json::to_value(&restored.btech).unwrap();
    encoded["constructed"][source.0.to_string()]["brief"]["contacts"] = serde_json::json!(4);
    // Deserialization is structural; world validation enforces the mode range on load.
    let mut corrupted = restored.clone();
    corrupted.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        persistence::save(&config.database(), &corrupted)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn contact_modes_order_buildings_wrecks_and_units_without_changing_lua_queries() {
    let (_dir, config, mut world, map, source, far) = fixture().await;
    place_battle_unit(&mut world, far, map, 1, 4).unwrap();
    acquire(&mut world, source, far);
    let near = world.create(&config, "Near unit".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        near,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, near, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, near, map, 1, 2).unwrap();
    acquire(&mut world, source, near);
    let building = scan_structure(&mut world, &config, map);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    let order = |text: &str, ids: &[ObjectId]| {
        let positions: Vec<_> = ids
            .iter()
            .map(|id| {
                text.find(&format!("#{} ", id.0))
                    .or_else(|| {
                        scripts
                            .world()
                            .btech
                            .constructed_units()
                            .get(id)
                            .and_then(|unit| {
                                let label = unit.battlefield_id()?.to_ascii_lowercase();
                                text.find(&format!("[{label}]"))
                            })
                    })
                    .or_else(|| {
                        scripts
                            .world()
                            .objects
                            .get(id)
                            .and_then(|object| text.find(&text::plain(&object.name)))
                    })
                    .unwrap_or_else(|| panic!("Missing #{} in {text}", id.0))
            })
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    };
    order(
        &support::run_text(&scripts, &config, ObjectId(1), 1, "contacts"),
        &[building, far, near],
    );
    assert_eq!(scripts.world().btech, before);
    let mut encoded = serde_json::to_value(&scripts.world().btech).unwrap();
    encoded["constructed"][near.0.to_string()]["pilot_injuries"] = serde_json::json!(6);
    encoded["constructed"][near.0.to_string()]["pilot_killed"] = true.into();
    shared.borrow_mut().btech = serde_json::from_value(encoded).unwrap();
    for mode in 1..=3 {
        battle_brief(&scripts, source, ObjectId(1), &format!("C {mode}")).unwrap();
        let _ = scripts.drain_outbox();
        order(
            &support::run_text(&scripts, &config, ObjectId(1), 1, "contacts bdsae"),
            &[building, near, far],
        );
    }
    battle_brief(&scripts, source, ObjectId(1), "C 0").unwrap();
    let _ = scripts.drain_outbox();
    order(
        &support::run_text(&scripts, &config, ObjectId(1), 1, "contacts bdsae"),
        &[far, near, building],
    );
    let first: i64 = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({})[1].target",
            source.0
        ))
        .unwrap();
    assert_eq!(first, near.0);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    order(
        &support::run_text(&restarted, &config, ObjectId(1), 1, "contacts bdsae"),
        &[far, near, building],
    );
}

/// Contact status columns share precedence in native rows and Lua, and vanish when unperceived.
#[tokio::test]
async fn contact_status_columns_share_precedence_with_native_and_lua() {
    let (_dir, config, world, map, source, target) = fixture().await;
    let baseline = serde_json::to_value(&world.btech).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let cases = [
        (serde_json::json!({}), "   S "),
        (
            serde_json::json!({"power":{"state":"starting","remaining":3}}),
            "   s ",
        ),
        (serde_json::json!({"power":{"state":"running"}}), "     "),
        (serde_json::json!({"posture":"prone"}), "  FS "),
        (
            serde_json::json!({"pilot_injuries":6,"pilot_killed":true,"posture":"prone"}),
            " DFS ",
        ),
        (
            serde_json::json!({"power":{"state":"running"},"inferno_remaining":10}),
            " l I ",
        ),
        (
            serde_json::json!({"power":{"state":"running"},"inferno_remaining":10,"heat":{"stored":3.0,"excess":3.0}}),
            " l + ",
        ),
        (
            serde_json::json!({"beacons":{"CenterTorso":["narc"]}}),
            "   Sn",
        ),
        (
            serde_json::json!({"beacons":{"CenterTorso":["homing"]},"signature":{"team":99,"hidden":false,"illuminated":false}}),
            "   SN",
        ),
        (
            serde_json::json!({"electronics":{"guardian":"off","angel":"off","field":{"protected":true,"angel_protected":false,"disturbed":true,"angel_disturbed":false,"countered":false}}}),
            "   Sp",
        ),
        (
            serde_json::json!({"electronics":{"guardian":"off","angel":"off","field":{"protected":false,"angel_protected":false,"disturbed":true,"angel_disturbed":false,"countered":false}}}),
            "   Se",
        ),
    ];
    for (fields, expected) in cases {
        let mut state = baseline.clone();
        for (key, value) in fields.as_object().unwrap() {
            state["constructed"][target.0.to_string()][key] = value.clone();
        }
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let before = scripts.world().btech.clone();
        let contact = visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .unwrap();
        assert_eq!(contact.status, expected);
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("contacts #{}", target.0),
        );
        assert!(
            text::plain(&text)
                .lines()
                .any(|line| line.ends_with(&format!("S:{expected}"))),
            "{text}"
        );
        let status: String = scripts
            .eval_callback(&format!(
                "return btech.unit.contacts({})[1].status",
                source.0
            ))
            .unwrap();
        assert_eq!(status, expected);
        assert_eq!(scripts.world().btech, before);
    }
    shared.borrow_mut().btech = serde_json::from_value(baseline).unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        visible_battle_contact(&restored, source, target)
            .unwrap()
            .unwrap()
            .status,
        "   S "
    );
    blind(&mut shared.borrow_mut(), map);
    assert!(
        visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .is_none()
    );
}

/// Contact views recheck which channel reaches the target from current map conditions without
/// rewriting the saved acquisition, and native rows, Lua and restart agree on the channel.
#[tokio::test]
async fn contact_detection_rechecks_channels_without_rewriting_acquisition() {
    let (_dir, config, world, map, source, target) = fixture().await;
    let baseline = world.btech.clone();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    for (sensors, visibility, expected) in [
        (true, 30, Some(BattleDetectionChannel::Sensors)),
        (true, 0, Some(BattleDetectionChannel::Sensors)),
        (false, 30, Some(BattleDetectionChannel::Sight)),
        (false, 0, None),
    ] {
        shared.borrow_mut().btech = baseline.clone();
        set_battle_map_perception(
            &mut shared.borrow_mut(),
            map,
            BattleMapPerceptionFlag::Sensors,
            sensors,
        )
        .unwrap();
        set_battle_map_visibility(&mut shared.borrow_mut(), map, BattleLight::Day, visibility)
            .unwrap();
        let before = scripts.world().btech.clone();
        let contact = visible_battle_contact(&scripts.world(), source, target).unwrap();
        assert_eq!(contact.as_ref().map(|c| c.detection), expected.map(Some));
        if let Some(channel) = expected {
            let contact = contact.unwrap();
            let text = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("contacts #{}", target.0),
            );
            assert!(
                text::plain(&text).contains(&format!(
                    "{} {}[{}]",
                    channel.code(true),
                    contact.weapon_arc.symbol(),
                    contact.label
                )),
                "{text}"
            );
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.unit.contacts({})[1].detection",
                    source.0
                ))
                .unwrap();
            assert_eq!(lua, channel.name());
        }
        assert_eq!(scripts.world().btech, before);
        assert!(
            scripts.world().btech.constructed_units()[&source]
                .contacts()
                .contains_key(&target)
        );
    }
    shared.borrow_mut().btech = baseline;
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        visible_battle_contact(&restored, source, target)
            .unwrap()
            .unwrap()
            .detection,
        Some(BattleDetectionChannel::Sensors)
    );
    let mut state = serde_json::to_value(&restored.btech).unwrap();
    state["constructed"][source.0.to_string()]["contacts"] = serde_json::json!({});
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert!(
        visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .is_none()
    );
}

/// Contact weapon arcs follow observer heading and torso twist in native rows, Lua and restart.
#[tokio::test]
async fn contact_weapon_arc_tracks_observer_pose_in_native_lua_and_restart() {
    let (_dir, config, world, _map, source, target) = fixture().await;
    let baseline = serde_json::to_value(&world.btech).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    for (heading, torso, expected) in [
        (0.0, BattleTorso::Center, BattleContactArc::Rear),
        (90.0, BattleTorso::Center, BattleContactArc::Right),
        (270.0, BattleTorso::Center, BattleContactArc::Left),
        (180.0, BattleTorso::Center, BattleContactArc::Front),
        (90.0, BattleTorso::Right, BattleContactArc::Front),
        (90.0, BattleTorso::Both, BattleContactArc::Front),
        (90.0, BattleTorso::Left, BattleContactArc::Rear),
    ] {
        let mut state = baseline.clone();
        state["constructed"][source.0.to_string()]["motion"]["heading"] =
            serde_json::json!(heading);
        state["constructed"][source.0.to_string()]["facing"] = serde_json::to_value(BattleFacing {
            torso,
            arms_flipped: false,
        })
        .unwrap();
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let before = scripts.world().btech.clone();
        let contact = visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .unwrap();
        assert_eq!(contact.weapon_arc, expected);
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("contacts #{}", target.0),
        );
        assert!(
            text::plain(&text).contains(&format!("S {}[{}]", expected.symbol(), contact.label)),
            "{text}"
        );
        let arc: String = scripts
            .eval_callback(&format!(
                "return btech.unit.contacts({})[1].weapon_arc",
                source.0
            ))
            .unwrap();
        assert_eq!(
            serde_json::json!(arc),
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(scripts.world().btech, before);
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        visible_battle_contact(&restored, source, target)
            .unwrap()
            .unwrap()
            .weapon_arc,
        BattleContactArc::Rear
    );
}

#[tokio::test]
async fn automatic_contact_modes_control_detail_color_and_escaping() {
    let (_dir, config, mut world, _map, source, target) = fixture().await;
    set_battle_autocon_shutdown(&mut world, source, ObjectId(1), true).unwrap();
    let baseline = serde_json::to_value(&world.btech).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let event = BattleContactEvent {
        experience_message: None,
        identified: true,
        observer: source,
        target,
        acquired: true,
        lock_lost: false,
    };
    for friendly in [true, false] {
        for mode in 0..=6 {
            let mut state = baseline.clone();
            state["constructed"][source.0.to_string()]["brief"]["automatic"] =
                serde_json::json!(mode);
            state["constructed"][target.0.to_string()]["signature"]["team"] =
                serde_json::json!(if friendly { 0 } else { 99 });
            shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
            let before = scripts.world().btech.clone();
            let expected = mode != 6 && !(friendly && matches!(mode, 2 | 3 | 5));
            for acquired in [true, false] {
                let notice = BattleContactEvent {
                    acquired,
                    ..event.clone()
                }
                .notice(&scripts.world());
                assert_eq!(notice.is_some(), expected);
                if let Some(notice) = notice {
                    let plain = text::plain(&notice.text);
                    let prefix = match (acquired, matches!(mode, 0 | 2)) {
                        (true, true) => "You notice ",
                        (true, false) => "Seen: ",
                        (false, true) => "You have lost ",
                        (false, false) => "Lost: ",
                    };
                    assert!(plain.starts_with(prefix), "{plain}");
                    assert!(plain.contains("Rear arc."));
                    assert!(
                        plain.contains(
                            &scripts.world().btech.constructed_units()[&target]
                                .definition()
                                .name
                        )
                    );
                    assert_eq!(notice.text.starts_with("[fg="), !friendly && mode < 4);
                    if !friendly && mode < 4 {
                        assert!(notice.text.starts_with(if acquired {
                            "[fg=red]"
                        } else {
                            "[fg=yellow]"
                        }));
                    }
                }
            }
            let warning = BattleContactEvent {
                acquired: false,
                lock_lost: true,
                ..event.clone()
            }
            .notice(&scripts.world())
            .unwrap();
            assert!(
                warning
                    .text
                    .ends_with("Weapon system reports the lock has been lost.")
            );
            assert_eq!(scripts.world().btech, before);
        }
    }
    let mut state = baseline;
    state["constructed"][target.0.to_string()]["definition"]["name"] =
        serde_json::json!("[fg=red]Forged[reset]");
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let notice = event.notice(&scripts.world()).unwrap();
    assert!(text::plain(&notice.text).contains("Forged"));
    assert!(!notice.text.contains("[fg=red]"));
}

/// Loss notices name the target only when the saved contact was identified before removal.
#[tokio::test]
async fn loss_notices_preserve_saved_identification_before_contact_removal() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    set_battle_autocon_shutdown(&mut world, source, ObjectId(1), true).unwrap();
    let _ = refresh_battle_contacts(&mut world, &[source]).unwrap();
    assert!(world.btech.constructed_units()[&source].contacts()[&target].identified);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(restored.btech.constructed_units()[&source].contacts()[&target].identified);
    for identified in [false, true] {
        let mut state = serde_json::to_value(&restored.btech).unwrap();
        state["constructed"][source.0.to_string()]["contacts"][target.0.to_string()]["identified"] =
            serde_json::json!(identified);
        let mut loss = restored.clone();
        loss.btech = serde_json::from_value(state).unwrap();
        blind(&mut loss, map);
        let events = refresh_battle_contacts(&mut loss, &[source]).unwrap();
        let event = events.iter().find(|event| event.target == target).unwrap();
        assert!(!event.acquired);
        assert_eq!(event.identified, identified);
        assert!(
            !loss.btech.constructed_units()[&source]
                .contacts()
                .contains_key(&target)
        );
        let notice = text::plain(&event.notice(&loss).unwrap().text);
        assert_eq!(notice.contains("something"), !identified);
        assert_eq!(
            notice.contains(&loss.btech.constructed_units()[&target].definition().name),
            identified
        );
        assert!(
            refresh_battle_contacts(&mut loss, &[source])
                .unwrap()
                .is_empty()
        );
    }
}

/// A probe contact behind a ridge stays unidentified: its identity, status and friendly
/// category are hidden in native rows, Lua and after restart.
#[tokio::test]
async fn probe_contacts_through_terrain_hide_identity_and_friendly_categories() {
    let (_dir, config, mut world, _old_map, source, target) = fixture().await;
    let map = world.create(&config, "Ridge field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ridge.map",
        BattleMapAsset::from_cells("3 5\n.0.0.0\n.0.0.0\n.9.9.9\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let _ = stop_battle_unit(
        &mut world,
        source,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(&mut world, source, map, 1, 1).unwrap();
    assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, source, ObjectId(1), true).unwrap();
    place_battle_unit(&mut world, target, map, 1, 3).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        let _ = advance_battle_units(&mut world, 0);
    }
    world
        .btech
        .rewrite_unit_record(source, |record| {
            record["definition"]["sections"]["LeftTorso"]["criticals"]["8"] =
                serde_json::json!({"equipment":"BeagleProbe","data":"-","modes":[]});
        })
        .unwrap();
    assert!(
        battle_unit_terrain_los(&world, source, target)
            .unwrap()
            .blocked
    );
    acquire(&mut world, source, target);
    let _ = refresh_battle_contacts(&mut world, &[source]).unwrap();
    assert!(!world.btech.constructed_units()[&source].contacts()[&target].identified);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    let view = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert!(!view.identified && !view.friendly);
    assert_eq!(view.name, "something");
    assert_eq!(view.status, "     ");
    assert_eq!(view.detection, Some(BattleDetectionChannel::Probe));
    assert!(view.short_text.starts_with("p "), "{}", view.short_text);
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
    assert!(native.contains("something"));
    assert!(
        !native.contains(
            &scripts.world().btech.constructed_units()[&target]
                .definition()
                .name
        )
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts a"),
        "Line of Sight Contacts:\r\nEnd Contact List"
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts e").contains("something")
    );
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.contacts({})[1]", source.0))
        .unwrap();
    assert_eq!(lua.get::<bool>("identified").unwrap(), view.identified);
    assert_eq!(lua.get::<bool>("friendly").unwrap(), view.friendly);
    assert_eq!(lua.get::<String>("name").unwrap(), view.name);
    assert_eq!(lua.get::<String>("status").unwrap(), view.status);
    assert_eq!(
        lua.get::<String>("detection").unwrap(),
        BattleDetectionChannel::Probe.name()
    );
    let range: mlua::Table = lua.get("range").unwrap();
    assert_eq!(range.get::<f64>("spatial").unwrap(), view.range.spatial);
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        visible_battle_contact(&restarted.world(), source, target)
            .unwrap()
            .unwrap(),
        view
    );
}

#[tokio::test]
async fn shutdown_contact_notice_preference_is_independent_and_durable() {
    let (_dir, config, world, _map, source, target) = fixture().await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let event = BattleContactEvent {
        experience_message: None,
        identified: true,
        observer: source,
        target,
        acquired: true,
        lock_lost: false,
    };
    assert!(!scripts.world().btech.constructed_units()[&source].autocon_shutdown());
    assert!(event.notice(&scripts.world()).is_none());
    assert!(
        visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .is_some()
    );
    let lock = BattleContactEvent {
        acquired: false,
        lock_lost: true,
        ..event.clone()
    };
    assert_eq!(
        lock.notice(&scripts.world()).unwrap().text,
        "Weapon system reports the lock has been lost."
    );
    let native = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "mechprefs AutoconShutdown ON",
    );
    assert!(
        native.contains("Autocon on shutdown units turned ON"),
        "{native}"
    );
    assert!(event.notice(&scripts.world()).is_some());
    let enabled = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.autocon_shutdown({},1,false); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, enabled);
    let _: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.autocon_shutdown({},1,false)",
            source.0
        ))
        .unwrap();
    assert!(event.notice(&scripts.world()).is_none());
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert!(event.notice(&scripts.world()).is_some());
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Starting { remaining: 3 }).unwrap();
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert!(event.notice(&scripts.world()).is_none());
    shared.borrow_mut().btech = enabled;
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(restored.btech.constructed_units()[&source].autocon_shutdown());
    assert_eq!(event.notice(&restored), event.notice(&saved));
    assert_eq!(
        battle_contact_preferences(&restored, ObjectId(1)).unwrap(),
        BattleContactPreferences::default()
    );
    assert!(
        set_battle_autocon_shutdown(&mut shared.borrow_mut(), source, ObjectId(2), false).is_err()
    );
}

/// Compact contact rows lead with the detection code and keep fixed columns and bounded names.
#[tokio::test]
async fn compact_contact_rows_have_fixed_columns_and_bounded_names() {
    let (_dir, config, world, _map, source, target) = fixture().await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let row = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    let expected = "S v[ab]B Jenner       x:  1 y:  2 z:  0 r: 1.0 b:180 s:  0.0 h:  0 S:   S ";
    assert_eq!(row.short_text, expected);
    assert_eq!(row.label, "ab");
    assert_eq!(row.coordinate, BattleHexCoordinate { x: 1, y: 2 });
    assert_eq!(row.elevation, 0);
    for mode in 1..=3 {
        battle_brief(&scripts, source, ObjectId(1), &format!("C {mode}")).unwrap();
        let _ = scripts.drain_outbox();
        assert!(
            text::plain(&support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                "contacts"
            ))
            .contains(expected)
        );
    }
    let text: String = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({})[1].short_text",
            source.0
        ))
        .unwrap();
    assert_eq!(text, expected);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        visible_battle_contact(&restored, source, target)
            .unwrap()
            .unwrap()
            .short_text,
        expected
    );
    let mut state = serde_json::to_value(&saved.btech).unwrap();
    state["constructed"][target.0.to_string()]["definition"]["name"] =
        serde_json::json!("[fg=red]Very Long Unit Name[reset]");
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let row = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert!(row.short_text.starts_with("S v[ab]B Very Long Un x:"));
    assert!(!row.short_text.contains("[fg="));
}

/// Building rows lead with the channel that reaches the entrance hex, shared by native, Lua and
/// restart, and disappear once neither the sensor band nor sight reaches it.
#[tokio::test]
async fn compact_building_rows_share_terrain_detection_channels() {
    let (_dir, config, mut world, map, source, _target) = fixture().await;
    let _ = scan_structure(&mut world, &config, map);
    let baseline = world.btech.clone();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let expected = "S v Hangar                  x:  1 y:  2 z: 1 r: 1.0 b:180 CF:  31 /  50 S:  ";
    let contacts = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
    assert_eq!(contacts[0].short_text, expected);
    assert_eq!(contacts[0].text(), expected);
    assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b").contains(expected));
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.building_contacts({},1)[1].short_text",
            source.0
        ))
        .unwrap();
    assert_eq!(lua, expected);
    for (sensors, visibility, expected) in [
        (true, 0, Some(BattleDetectionChannel::Sensors)),
        (false, 30, Some(BattleDetectionChannel::Sight)),
        (false, 0, None),
    ] {
        shared.borrow_mut().btech = baseline.clone();
        set_battle_map_perception(
            &mut shared.borrow_mut(),
            map,
            BattleMapPerceptionFlag::Sensors,
            sensors,
        )
        .unwrap();
        set_battle_map_visibility(&mut shared.borrow_mut(), map, BattleLight::Day, visibility)
            .unwrap();
        let before = scripts.world().btech.clone();
        let detection =
            battle_hex_perception(&scripts.world(), source, BattleHexCoordinate { x: 1, y: 2 })
                .unwrap();
        assert_eq!(detection, expected);
        let contacts = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
        assert_eq!(contacts.is_empty(), expected.is_none());
        if let Some(contact) = contacts.first() {
            assert_eq!(contact.detection, detection);
            assert!(
                contact
                    .short_text
                    .starts_with(&format!("{} v ", detection.unwrap().code(true)))
            );
        }
        assert_eq!(scripts.world().btech, before);
    }
    shared.borrow_mut().btech = baseline;
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        battle_building_contacts(&restarted, source, ObjectId(1)).unwrap()[0].short_text,
        expected
    );
}

#[tokio::test]
async fn contact_colors_prioritize_selection_and_escape_row_data() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    let _ = scan_structure(&mut world, &config, map);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let view = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert_eq!(
        view.styled_short_text(false),
        text::escape(&view.short_text)
    );
    assert!(view.styled_short_text(false).contains("[[ab]"));
    let _ =
        select_battle_target(&mut shared.borrow_mut(), source, ObjectId(1), Some(target)).unwrap();
    let selected = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
    assert!(selected.contains(&view.styled_short_text(true)));
    assert!(view.styled_short_text(true).starts_with("[fg=red bold]"));
    let mut signature = scripts.world().btech.constructed_units()[&target].signature();
    signature.team = 99;
    set_battle_unit_signature(&mut shared.borrow_mut(), target, signature).unwrap();
    let hostile = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts")
            .contains(&hostile.styled_short_text(true))
    );
    let _ = select_battle_target(&mut shared.borrow_mut(), source, ObjectId(1), None).unwrap();
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
    assert!(native.contains(&hostile.styled_short_text(false)));
    assert!(
        hostile
            .styled_short_text(false)
            .starts_with("[fg=yellow bold]")
    );
    assert_eq!(
        text::plain(&hostile.styled_short_text(false)),
        hostile.short_text
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({})[1].short_text",
            source.0
        ))
        .unwrap();
    assert_eq!(lua, hostile.short_text);
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set("test_parents", parents)
        .unwrap();
    scripts.eval_callback::<()>("test_parents['default_room.lua'].locks={identify_building=function(ctx) return false end}").unwrap();
    let buildings = battle_building_contacts(&scripts, source, ObjectId(1)).unwrap();
    assert!(buildings[0].styled_text().starts_with("[fg=yellow bold]"));
    assert_eq!(
        text::plain(&buildings[0].styled_text()),
        buildings[0].short_text
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts b")
            .contains(&buildings[0].styled_text())
    );
    let action = commands::run(&scripts, &config, ObjectId(1), 1, "contacts").unwrap();
    assert!(matches!(
        action,
        CommandAction::Report(CommandReport::Styled(_))
    ));
}

#[tokio::test]
async fn contact_framing_handles_empty_lists_diagnostics_and_restart() {
    let (_dir, config, world, _map, source, _target) = fixture().await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared).unwrap();
    for mode in 0..=3 {
        battle_brief(&scripts, source, ObjectId(1), &format!("C {mode}")).unwrap();
        let _ = scripts.drain_outbox();
        let before = scripts.world().btech.clone();
        let filled = text::plain(&support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "contacts",
        ));
        let empty = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts t");
        let diagnostic = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts tQ");
        if mode == 3 {
            assert!(!filled.contains("Line of Sight Contacts:"));
            assert!(!filled.contains("End Contact List"));
            assert!(!filled.is_empty());
            assert_eq!(empty, "");
            assert_eq!(diagnostic, "Ignoring Q as contact option.");
        } else {
            assert!(filled.starts_with("Line of Sight Contacts:\r\n"));
            assert!(filled.ends_with("\r\nEnd Contact List"));
            assert_eq!(empty, "Line of Sight Contacts:\r\nEnd Contact List");
            assert_eq!(
                diagnostic,
                format!("Ignoring Q as contact option.\r\n{empty}")
            );
        }
        assert_eq!(scripts.world().btech, before);
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .brief_settings()
            .contacts,
        3
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "contacts t"),
        ""
    );
    assert!(
        !support::run_text(&scripts, &config, ObjectId(1), 1, "contacts")
            .contains("End Contact List")
    );
}

#[tokio::test]
async fn verbose_contacts_share_multiline_reports_with_lua_and_restart() {
    let (_dir, config, world, _map, source, target) = fixture().await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    battle_brief(&scripts, source, ObjectId(1), "C 0").unwrap();
    let _ = scripts.drain_outbox();
    let expected = concat!(
        "[ab] Jenner             Tonnage: 35\r\n",
        "      Range: 1.0 hex\tBearing: 180 degrees\r\n",
        "      Speed: 0.0 KPH\tHeading: 0 degrees\r\n",
        "      X, Y:   1,   2 \tHeat: 0 deg C.\r\n",
        "      Movement Type: BIPED\r\n",
        "      Mech is in Rear Arc\r\n",
        "      Mech Shutdown\r\n ",
    );
    let before = scripts.world().btech.clone();
    let row = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert_eq!(row.verbose_text, expected);
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts");
    assert_eq!(
        native,
        format!(
            "Line of Sight Contacts:\r\n{}\r\nEnd Contact List",
            text::escape(expected)
        )
    );
    let lua: String = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({})[1].verbose_text",
            source.0
        ))
        .unwrap();
    assert_eq!(lua, expected);
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let restarted =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    assert_eq!(
        support::run_text(&restarted, &config, ObjectId(1), 1, "contacts"),
        native
    );

    let mut state = serde_json::to_value(&before).unwrap();
    let unit = &mut state["constructed"][target.0.to_string()];
    unit["definition"]["name"] = serde_json::json!("[fg=red]Long [Name] Beyond Seventeen[reset]");
    unit["pilot_injuries"] = serde_json::json!(6);
    unit["pilot_killed"] = true.into();
    unit["posture"] = serde_json::json!("prone");
    unit["heat"] = serde_json::json!({"stored":12.0,"excess":12.0});
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let row = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert!(
        row.verbose_text
            .starts_with("[ab] Long [Name] Beyond Seventeen  Tonnage: 35")
    );
    assert!(row.verbose_text.contains("Heat: 12 deg C."));
    assert!(
        row.verbose_text
            .ends_with("Mech Destroyed\r\n      Mech Shutdown\r\n      Mech has Fallen!\r\n ")
    );
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "contacts dsae");
    assert!(native.contains(&text::escape(&row.verbose_text)));
    assert!(!native.contains("[fg=red]"));

    let mut state = serde_json::to_value(&before).unwrap();
    let start = BattleHexCoordinate { x: 1, y: 2 }.center();
    let end = BattleHexCoordinate { x: 1, y: 4 }.center();
    let path = BattleJumpPath::new(start, end, 0, 0, 5).unwrap();
    state["constructed"][target.0.to_string()]["flight"] =
        serde_json::to_value(BattleJumpFlight::new(path)).unwrap();
    state["constructed"][target.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let row = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert!(!row.verbose_text.contains("Mech Shutdown"));
    assert!(
        row.verbose_text
            .ends_with("Mech is Jumping!\tJump Heading: 180\r\n ")
    );
}

#[tokio::test]
async fn lateral_changes_delay_cancel_persist_and_move_without_turning_weapons() {
    let (_dir, config, world, _map, source, target) =
        fixture_with_ranges(&[("move_type", "Quad")]).await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    assert!(battle_lateral(&scripts, source, ObjectId(2), "ne").is_err());
    assert!(battle_lateral(&scripts, source, ObjectId(1), "bad").is_err());
    let initial = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.lateral({},1,'ne'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, initial);
    let _ = support::run_text(&scripts, &config, ObjectId(1), 1, "lateral ne");
    let _ = scripts.drain_outbox();
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .lateral()
            .remaining,
        6
    );
    for _ in 0..3 {
        let _ = advance_battle_units(&mut shared.borrow_mut(), 0);
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        restored.btech.constructed_units()[&source]
            .lateral()
            .remaining,
        3
    );
    *shared.borrow_mut() = restored;
    for _ in 0..2 {
        let _ = advance_battle_units(&mut shared.borrow_mut(), 0);
    }
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .lateral()
            .active,
        BattleLateralMode::None
    );
    let notices = advance_battle_units(&mut shared.borrow_mut(), 0);
    assert!(notices.iter().any(|notice| notice.text
        == "Lateral movement mode change to Front/Right (60 offset) completed."));
    assert_eq!(
        scripts.world().btech.constructed_units()[&source].travel_heading(),
        Some(60.0)
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .motion()
            .unwrap()
            .heading,
        0.0
    );
    assert_eq!(
        visible_battle_contact(&scripts.world(), source, target)
            .unwrap()
            .unwrap()
            .weapon_arc,
        BattleContactArc::Rear
    );
    battle_lateral(&scripts, source, ObjectId(1), "sw").unwrap();
    let abort = battle_lateral(&scripts, source, ObjectId(1), "fr").unwrap();
    assert_eq!(abort.text, "Lateral mode change aborted.");
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .lateral()
            .remaining,
        0
    );
    assert!(battle_lateral(&scripts, source, ObjectId(1), "ne").is_err());
    battle_lateral(&scripts, source, ObjectId(1), "nw").unwrap();
    let _: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.lateral({},1,'rr')", source.0))
        .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .lateral()
            .pending,
        Some(BattleLateralMode::RearRight)
    );
    battle_lateral(&scripts, source, ObjectId(1), "fr").unwrap();
    let _ = scripts.drain_outbox();
    let point = scripts.world().btech.constructed_units()[&source]
        .motion()
        .unwrap()
        .point;
    let _ = set_battle_speed(&mut shared.borrow_mut(), source, ObjectId(1), 10.0).unwrap();
    let _ = advance_battle_motion(&mut shared.borrow_mut(), BattleMovementRules::STANDARD).unwrap();
    let unit = scripts.world().btech.constructed_units()[&source].clone();
    let bearing = point
        .bearing(unit.motion().unwrap().point)
        .unwrap()
        .unwrap();
    assert!((bearing - 60.0).abs() < 1e-7);
    assert_eq!(unit.motion().unwrap().heading, 0.0);

    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    state["constructed"][target.0.to_string()]["lateral"] =
        serde_json::to_value(unit.lateral()).unwrap();
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let contact = visible_battle_contact(&scripts.world(), source, target)
        .unwrap()
        .unwrap();
    assert_eq!(contact.heading, 60.0);
    assert!(contact.short_text.contains("h: 60"));
    assert!(contact.verbose_text.contains("Heading: 60 degrees"));
    let heading: f64 = scripts
        .eval_callback(&format!(
            "return btech.unit.contacts({})[1].heading",
            source.0
        ))
        .unwrap();
    assert_eq!(heading, 60.0);
    for (name, mode) in [
        ("nw", BattleLateralMode::FrontLeft),
        ("FL", BattleLateralMode::FrontLeft),
        ("ne", BattleLateralMode::FrontRight),
        ("fr", BattleLateralMode::FrontRight),
        ("sw", BattleLateralMode::RearLeft),
        ("rl", BattleLateralMode::RearLeft),
        ("se", BattleLateralMode::RearRight),
        ("rr", BattleLateralMode::RearRight),
        ("-", BattleLateralMode::None),
    ] {
        assert_eq!(BattleLateralMode::parse(name).unwrap(), mode);
    }
    // Stopping does not freeze the scheduled change or apply it while powered off.
    let mut stopped = scripts.world().clone();
    let _ = set_battle_lateral(
        &mut stopped,
        source,
        ObjectId(1),
        BattleLateralMode::RearLeft,
    )
    .unwrap();
    stopped
        .btech
        .rewrite_unit_record(source, |record| {
            record["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        })
        .unwrap();
    for _ in 0..5 {
        let _ = advance_battle_units(&mut stopped, 0);
    }
    assert_eq!(
        stopped.btech.constructed_units()[&source]
            .lateral()
            .remaining,
        1
    );
    assert!(advance_battle_units(&mut stopped, 0).is_empty());
    assert_eq!(
        stopped.btech.constructed_units()[&source].lateral().active,
        BattleLateralMode::FrontRight
    );
    assert_eq!(
        stopped.btech.constructed_units()[&source].lateral().pending,
        None
    );

    let mut corrupt = serde_json::to_value(&unit).unwrap();
    corrupt["lateral"]["remaining"] = serde_json::json!(7);
    let mut invalid = scripts.world().clone();
    let mut encoded = serde_json::to_value(&invalid.btech).unwrap();
    encoded["constructed"][source.0.to_string()] = corrupt;
    invalid.btech = serde_json::from_value(encoded).unwrap();
    assert!(
        persistence::save(&config.database(), &invalid)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn cockpit_status_reports_quad_lateral_travel() {
    let (_dir, config, world, _map, source, _target) =
        fixture_with_ranges(&[("move_type", "Quad")]).await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let initial = battle_unit_status(&scripts.world(), source, "info").unwrap();
    assert!(!initial.contains("Turn Mode:"));
    assert!(!initial.contains("moving laterally"));
    battle_lateral(&scripts, source, ObjectId(1), "fl").unwrap();
    assert!(
        !battle_unit_status(&scripts.world(), source, "info")
            .unwrap()
            .contains("moving laterally")
    );
    for _ in 0..6 {
        let _ = advance_battle_units(&mut shared.borrow_mut(), 0);
    }
    let _ = scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    let status = battle_unit_status(&scripts.world(), source, "info").unwrap();
    assert!(status.contains("You are moving laterally Front/Left"));
    assert!(!status.contains("Turn Mode:"));
    assert!(stompymux_rs::text::plain(&status).contains("Heading:        0 deg"));
    let native = support::run_text(&scripts, &config, ObjectId(1), 1, "status info");
    assert_eq!(native, status);
    let lua: String = scripts
        .eval_callback(&format!("return btech.unit.status({},'info')", source.0))
        .unwrap();
    assert_eq!(lua, status);
    let short = battle_unit_status(&scripts.world(), source, "short").unwrap();
    assert!(!short.contains("Turn Mode:"));
    assert!(!short.contains("moving laterally"));
    for options in ["armor", "heat", "N"] {
        let output = battle_unit_status(&scripts.world(), source, options).unwrap();
        assert!(!output.contains("Turn Mode:"));
        assert!(!output.contains("moving laterally"));
    }
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_unit_status(&restored, source, "info").unwrap(),
        status
    );
    let output = battle_unit_status(&scripts.world(), source, "info").unwrap();
    assert!(!output.contains("Turn Mode:"));
    assert!(output.contains("You are moving laterally Front/Left"));
}

#[tokio::test]
async fn bootlegger_pivots_recycle_legs_and_replay_failed_falls_atomically() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    let passenger = world.objects.get_mut(&ObjectId(2)).unwrap();
    passenger.location = Some(source);
    passenger.flags.insert(Flag::Connected);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let mut initial = serde_json::to_value(&scripts.world().btech).unwrap();
    let unit = &mut initial["constructed"][source.0.to_string()];
    unit["motion"]["heading"] = serde_json::json!(350.0);
    unit["motion"]["desired_heading"] = serde_json::json!(350.0);
    unit["motion"]["speed"] = serde_json::json!(50.0);
    unit["motion"]["desired_speed"] = serde_json::json!(50.0);
    for success in [true, false] {
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).two_d6() >= 7) == success)
            .unwrap();
        let mut state = initial.clone();
        state["constructed"][source.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        shared.borrow_mut().btech = serde_json::from_value(state.clone()).unwrap();
        assert_eq!(
            battle_bootlegger_modifier(&scripts.world(), source, ObjectId(1)).unwrap(),
            1
        );
        let before = scripts.world().btech.clone();
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.bootlegger({},1,'right'); error('abort')",
                    source.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let report = battle_bootlegger(&scripts, &config, source, ObjectId(1), "right").unwrap();
        assert_eq!(report.check.success, success);
        let output = scripts.drain_outbox();
        let pilot_output: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, text)| text.source())
            .collect();
        assert_eq!(pilot_output[0], "You make a piloting skill roll!");
        assert_eq!(
            pilot_output[1],
            format!(
                "Modified Pilot Skill: BTH {}\tRoll: {}",
                report.check.target,
                report.check.roll.unwrap()
            )
        );
        assert!(pilot_output[2].starts_with("You plant a foot"));
        assert!(output.iter().any(|(who, text)| {
            *who == ObjectId(2) && text.source().starts_with("You plant a foot")
        }));
        assert!(!output.iter().any(|(who, text)| {
            *who != ObjectId(1)
                && (text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:"))
        }));
        let fall_feedback = report
            .fall
            .as_ref()
            .and_then(|fall| fall.avoidance.as_ref())
            .and_then(|check| check.messages());
        assert_eq!(
            report.pilot_notices.len(),
            2 + fall_feedback.as_ref().map_or(0, |messages| messages.len())
        );
        assert!(
            report
                .pilot_notices
                .iter()
                .all(|notice| notice.pilot == ObjectId(1))
        );
        assert!(
            report.pilot_notices[..2]
                .iter()
                .all(|notice| notice.before_notice == 0)
        );
        if let Some(messages) = fall_feedback {
            for message in messages {
                assert!(pilot_output.contains(&message.as_str()));
            }
        }
        let after = scripts.world().clone();
        let unit = &after.btech.constructed_units()[&source];
        if success {
            let motion = unit.motion().unwrap();
            assert_eq!(motion.heading, 80.0);
            assert_eq!(motion.desired_heading, 80.0);
            assert_eq!(motion.speed, 25.0);
            assert_eq!(motion.desired_speed, 50.0);
            assert_eq!(unit.limb_recycle()[&BattleSection::LeftLeg], 30);
            assert_eq!(unit.limb_recycle()[&BattleSection::RightLeg], 30);
            assert!(report.fall.is_none());
        } else {
            assert_eq!(unit.posture(), BattlePosture::Prone);
            assert!(report.fall.as_ref().unwrap().damage > 0);
        }
        persistence::save(&config.database(), &after).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech.constructed_units()[&source], *unit);
        shared.borrow_mut().btech = before;
        let replay = battle_bootlegger(&scripts, &config, source, ObjectId(1), "right").unwrap();
        assert_eq!(replay, report);
        assert_eq!(scripts.world().btech, after.btech);
        if success {
            shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
            let _ = scripts.drain_outbox();
            let _ = support::run_text(&scripts, &config, ObjectId(1), 1, "bootlegger left");
            assert_eq!(
                scripts.world().btech.constructed_units()[&source]
                    .motion()
                    .unwrap()
                    .heading,
                260.0
            );
        }
    }
    for (field, value) in [("speed", 42.9), ("speed", -50.0)] {
        let mut state = initial.clone();
        state["constructed"][source.0.to_string()]["motion"][field] = serde_json::json!(value);
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let before = scripts.world().btech.clone();
        assert!(battle_bootlegger(&scripts, &config, source, ObjectId(1), "r").is_err());
        assert_eq!(scripts.world().btech, before);
    }
    shared.borrow_mut().btech = serde_json::from_value(initial.clone()).unwrap();
    assert!(battle_bootlegger(&scripts, &config, source, ObjectId(2), "r").is_err());
    for direction in ["", "r l", "north"] {
        assert!(battle_bootlegger(&scripts, &config, source, ObjectId(1), direction).is_err());
    }
    for (key, value) in [
        ("limb_recycle", serde_json::json!({"LeftLeg":1})),
        ("power", serde_json::to_value(BattlePower::Off).unwrap()),
    ] {
        let mut state = initial.clone();
        state["constructed"][source.0.to_string()][key] = value;
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        assert!(battle_bootlegger_modifier(&scripts.world(), source, ObjectId(1)).is_err());
    }
}

#[tokio::test]
async fn bootlegger_character_checks_award_xp_and_preserve_failed_action_rollback() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    world
        .objects
        .get_mut(&source)
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
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    for skill in ["Piloting-Biped", "Piloting-Battlemech"] {
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            skill,
            BattleCharacterValue {
                value: 2,
                ..Default::default()
            },
        )
        .unwrap();
    }
    let mut initial = serde_json::to_value(&world.btech).unwrap();
    initial["constructed"][source.0.to_string()]["motion"]["speed"] = serde_json::json!(50.0);
    initial["constructed"][source.0.to_string()]["motion"]["desired_speed"] =
        serde_json::json!(50.0);
    for success in [true, false] {
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).two_d6() >= 7) == success)
            .unwrap();
        let mut state = initial.clone();
        state["constructed"][source.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        let mut candidate = world.clone();
        candidate.btech = serde_json::from_value(state).unwrap();
        let shared = std::rc::Rc::new(std::cell::RefCell::new(candidate));
        let scripts = Scripts::new(&config, shared).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.bootlegger({},1,'r'); error('abort')",
                    source.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let report = battle_bootlegger(&scripts, &config, source, ObjectId(1), "r").unwrap();
        assert_eq!(report.check.target, 7);
        assert_eq!(report.check.success, success);
        assert_eq!(report.check.experience.is_some(), success);
        assert_eq!(report.fall.is_some(), !success);
    }
}

#[tokio::test]
async fn eta_uses_horizontal_range_absolute_speed_and_only_plain_hex_defaults() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(source);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let baseline = serde_json::to_value(&scripts.world().btech).unwrap();
    for (speed, expected) in [
        (0.0, None),
        (0.099, None),
        (-0.099, None),
        (0.1, Some(107)),
        (-0.1, Some(107)),
        (10.75, Some(1)),
        (-10.75, Some(1)),
    ] {
        let mut state = baseline.clone();
        state["constructed"][source.0.to_string()]["motion"]["speed"] = serde_json::json!(speed);
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let before = scripts.world().btech.clone();
        let report = battle_eta(&scripts.world(), source, ObjectId(1), "1 2").unwrap();
        assert_eq!(report.range, 1.0);
        assert_eq!(report.minutes, expected);
        assert_eq!(scripts.world().btech, before);
        assert_eq!(
            battle_eta(&scripts.world(), source, ObjectId(2), "1 2").unwrap(),
            report
        );
        let native = support::run_text(&scripts, &config, ObjectId(1), 1, "eta 1 2");
        assert_eq!(native.lines().count(), 2);
        assert!(native.lines().all(|line| line == report.text));
        let lua: String = scripts
            .eval_callback(&format!("return btech.unit.eta({},1,'1 2').text", source.0))
            .unwrap();
        assert_eq!(lua, report.text);
        let _ = scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.eta({},1,'1 2'); error('abort')",
                    source.0
                ))
                .is_err()
        );
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, before);
    }
    assert_eq!(
        battle_eta(
            &scripts.world(),
            source,
            ObjectId(1),
            "2147483647 2147483647"
        )
        .unwrap()
        .minutes,
        Some(i32::MAX as u32)
    );
    let report = battle_eta(&scripts.world(), source, ObjectId(1), "1 61").unwrap();
    assert_eq!(report.text, "Range to hex (1,61) is 60.0.  ETA: 01:00.");
    assert_eq!(
        battle_eta(&scripts.world(), source, ObjectId(1), "1 1")
            .unwrap()
            .minutes,
        Some(0)
    );
    for arguments in ["", "1", "1 2 3", "bad 2", "1 2147483648"] {
        assert!(battle_eta(&scripts.world(), source, ObjectId(1), arguments).is_err());
    }
    assert!(battle_eta(&scripts.world(), source, ObjectId(3), "1 2").is_err());
    for mode in [
        BattleHexTargetMode::Hex,
        BattleHexTargetMode::Ignite,
        BattleHexTargetMode::Clear,
        BattleHexTargetMode::Building,
        BattleHexTargetMode::UnitAtHex,
    ] {
        let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
        state["constructed"][source.0.to_string()]["target_lock"] =
            serde_json::to_value(BattleTargetSelection::Hex(BattleHexLock {
                hex: BattleHexCoordinate { x: 1, y: 2 },
                mode,
                remaining: 3,
            }))
            .unwrap();
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let result = battle_eta(&scripts.world(), source, ObjectId(1), "");
        assert_eq!(result.is_ok(), mode == BattleHexTargetMode::Hex);
    }
    let _ = select_battle_hex_target(
        &mut shared.borrow_mut(),
        source,
        ObjectId(1),
        BattleHexCoordinate { x: 1, y: 2 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_eta(&restored, source, ObjectId(1), "").unwrap(),
        battle_eta(&scripts.world(), source, ObjectId(1), "").unwrap()
    );
}

/// Bearing queries share defaults and bounds and require a live view of the target.
#[tokio::test]
async fn bearing_queries_share_defaults_bounds_and_live_visibility_without_mutation() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(source);
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    for (arguments, expected) in [
        ("", "Bearing to default target is: 180 degrees."),
        ("1 2", "Bearing to  1,2 is: 180 degrees."),
        ("1 1", "Bearing to  1,1 is: 180 degrees."),
        ("1 2 1 1", "Bearing to 1,1 from 1,2 is: 0 degrees."),
        ("3 1 1 1", "Bearing to 1,1 from 3,1 is: 270 degrees."),
    ] {
        let report = battle_bearing(&scripts.world(), source, ObjectId(1), arguments).unwrap();
        assert_eq!(report.text, expected);
        assert_eq!(
            battle_bearing(&scripts.world(), source, ObjectId(2), arguments).unwrap(),
            report
        );
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("bearing {arguments}")
            ),
            expected
        );
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.bearing({},1,{:?}).text",
                source.0, arguments
            ))
            .unwrap();
        assert_eq!(lua, expected);
    }
    for arguments in [
        "1",
        "1 2 3",
        "1 2 1 2 3",
        "3 1",
        "4 1 1 1",
        "-1 1",
        "1 60",
        "1.5 1",
        "2147483648 1",
    ] {
        assert!(
            battle_bearing(&scripts.world(), source, ObjectId(1), arguments).is_err(),
            "{arguments}"
        );
    }
    assert!(battle_bearing(&scripts.world(), source, ObjectId(3), "1 2").is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_bearing(&restored, source, ObjectId(1), "").unwrap(),
        battle_bearing(&scripts.world(), source, ObjectId(1), "").unwrap()
    );
    blind(&mut shared.borrow_mut(), map);
    assert!(
        battle_bearing(&scripts.world(), source, ObjectId(1), "")
            .unwrap_err()
            .to_string()
            .contains("line of sight")
    );
    assert_eq!(
        battle_bearing(&scripts.world(), source, ObjectId(1), "1 2")
            .unwrap()
            .bearing,
        180
    );
    for mode in [
        BattleHexTargetMode::Hex,
        BattleHexTargetMode::Building,
        BattleHexTargetMode::Ignite,
        BattleHexTargetMode::Clear,
        BattleHexTargetMode::UnitAtHex,
    ] {
        let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
        state["constructed"][source.0.to_string()]["target_lock"] =
            serde_json::to_value(BattleTargetSelection::Hex(BattleHexLock {
                hex: BattleHexCoordinate { x: 1, y: 0 },
                mode,
                remaining: 3,
            }))
            .unwrap();
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        assert_eq!(
            battle_bearing(&scripts.world(), source, ObjectId(1), "")
                .unwrap()
                .bearing,
            0
        );
    }
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    let point = scripts.world().btech.constructed_units()[&source]
        .motion()
        .unwrap()
        .point;
    state["constructed"][source.0.to_string()]["motion"]["point"]["y"] =
        serde_json::json!(point.y + 0.25);
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        battle_bearing(&scripts.world(), source, ObjectId(1), "1 1")
            .unwrap()
            .bearing,
        0
    );
}

/// Range reports refuse unperceived targets but keep live target elevation otherwise.
#[tokio::test]
async fn range_reports_mask_dark_terrain_but_preserve_live_target_elevation() {
    let (_dir, config, world, map, source, target) = fixture().await;
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let baseline = serde_json::to_value(&scripts.world().btech).unwrap();
    for terrain in [Terrain::Water, Terrain::Ice, Terrain::Grassland] {
        for dark in [false, true] {
            let mut state = baseline.clone();
            state["maps"][map.0.to_string()]["terrain"][7] =
                serde_json::to_value(BattleHex::new(terrain, 5)).unwrap();
            state["maps"][map.0.to_string()]["terrain"][10] =
                serde_json::to_value(BattleHex::new(Terrain::Grassland, 5)).unwrap();
            state["maps"][map.0.to_string()]["flags"] =
                serde_json::json!(if dark { 32 } else { 0 });
            shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
            let before = scripts.world().btech.clone();
            let report = battle_range_report(&scripts.world(), source, ObjectId(1), "1 2").unwrap();
            assert_eq!(report.horizontal, 1.0);
            let expected = if dark {
                "Range to  1,2 is: 1.0 hexes."
            } else {
                "Range to  1,2 is: 1.4 hexes (1.0 ground hexes)."
            };
            assert_eq!(report.text, expected);
            let explicit =
                battle_range_report(&scripts.world(), source, ObjectId(1), "1 1 1 2").unwrap();
            assert_eq!(explicit.spatial, report.spatial);
            let across =
                battle_range_report(&scripts.world(), source, ObjectId(1), "1 2 1 3").unwrap();
            assert_eq!(
                across.spatial,
                if !dark && terrain != Terrain::Grassland {
                    5.0_f64.sqrt()
                } else {
                    1.0
                }
            );

            assert_eq!(
                support::run_text(&scripts, &config, ObjectId(1), 1, "range 1 2"),
                expected
            );
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.unit.range_report({},1,'1 2').text",
                    source.0
                ))
                .unwrap();
            assert_eq!(lua, expected);
            assert_eq!(scripts.world().btech, before);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
    shared.borrow_mut().btech = serde_json::from_value(baseline.clone()).unwrap();
    assert!(battle_range_report(&scripts.world(), source, ObjectId(1), "").is_err());
    for input in ["1", "1 2 3", "bad 1", "3 1", "1 60", "3 1 1 1"] {
        assert!(battle_range_report(&scripts.world(), source, ObjectId(1), input).is_err());
    }
    let _ =
        select_battle_target(&mut shared.borrow_mut(), source, ObjectId(1), Some(target)).unwrap();
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    state["constructed"][target.0.to_string()]["ground_elevation"] = serde_json::json!(5);
    state["maps"][map.0.to_string()]["flags"] = serde_json::json!(32);
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let report = battle_range_report(&scripts.world(), source, ObjectId(1), "").unwrap();
    assert_eq!(
        report.text,
        "Range to default target is: 1.4 hexes (1.0 ground hexes)."
    );
    // Existing unit-to-unit Lua geometry remains a separate, compatible operation.
    let spatial: f64 = scripts
        .eval_callback(&format!(
            "return btech.unit.range({},{}).spatial",
            source.0, target.0
        ))
        .unwrap();
    assert_eq!(spatial, report.spatial);
    blind(&mut shared.borrow_mut(), map);
    assert!(battle_range_report(&scripts.world(), source, ObjectId(1), "").is_err());
    assert!(battle_range_report(&scripts.world(), source, ObjectId(1), "1 2").is_ok());
    shared.borrow_mut().btech = serde_json::from_value(baseline).unwrap();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_range_report(&restored, source, ObjectId(1), "1 2").unwrap(),
        battle_range_report(&scripts.world(), source, ObjectId(1), "1 2").unwrap()
    );
}

/// Vector reports combine signed heights and bearings for every coordinate form.
#[tokio::test]
async fn vector_reports_combine_signed_heights_bearings_and_all_coordinate_forms() {
    let (_dir, config, mut world, map, source, target) = fixture().await;
    let _ = select_battle_target(&mut world, source, ObjectId(1), Some(target)).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    for (input, vertical, expected) in [
        (
            "",
            0,
            "Vector to default target is: 1.0 hexes and 180 degrees mark  0.",
        ),
        ("1 2", 0, "Vector to  1,2 is: 1.0 hexes and 180 degrees."),
        (
            "1 2 5",
            45,
            "Vector to  1,2,5 is: 1.4 hexes (1.0 ground hexes) and 180 degrees mark +45.",
        ),
        (
            "1 2 -5",
            -45,
            "Vector to  1,2,-5 is: 1.4 hexes (1.0 ground hexes) and 180 degrees mark -45.",
        ),
        (
            "1 1 5",
            90,
            "Vector to  1,1,5 is: 1.0 hexes (0.0 ground hexes) and 180 degrees mark +90.",
        ),
        (
            "1 2 1 1",
            0,
            "Vector to 1,1 from 1,2 is: 1.0 hexes and 0 degrees.",
        ),
        (
            "1 1 5 1 2 -5",
            -64,
            "Vector to 1,2,-5 from 1,1,5 is: 2.2 hexes (1.0 ground hexes) and 180 degrees mark -64.",
        ),
    ] {
        let report = battle_vector_report(&scripts.world(), source, ObjectId(1), input).unwrap();
        assert_eq!(report.vertical_bearing, vertical);
        assert_eq!(report.text, expected);
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("vector {input}")
            ),
            expected
        );
        let lua: String = scripts
            .eval_callback(&format!(
                "return btech.unit.vector({},1,{:?}).text",
                source.0, input
            ))
            .unwrap();
        assert_eq!(lua, expected);
    }
    for input in [
        "1",
        "1 1 1 1 1",
        "1 1 1 1 1 1 1",
        "1 1 x",
        "3 1 0",
        "1 60",
        "1 1 2147483648",
        "-1 1 0 1 1 0",
    ] {
        assert!(
            battle_vector_report(&scripts.world(), source, ObjectId(1), input).is_err(),
            "{input}"
        );
    }
    // Explicit heights permit the east-border origin without attempting to read a terrain tile.
    assert!(battle_vector_report(&scripts.world(), source, ObjectId(1), "3 1 0 1 1 0").is_ok());
    assert!(battle_vector_report(&scripts.world(), source, ObjectId(1), "3 1 1 1").is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_vector_report(&restored, source, ObjectId(1), "").unwrap(),
        battle_vector_report(&scripts.world(), source, ObjectId(1), "").unwrap()
    );
    let mut state = serde_json::to_value(&before).unwrap();
    state["maps"][map.0.to_string()]["flags"] = serde_json::json!(32);
    state["maps"][map.0.to_string()]["terrain"][7] =
        serde_json::to_value(BattleHex::new(Terrain::Ice, 5)).unwrap();
    shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
    let vector = battle_vector_report(&scripts.world(), source, ObjectId(1), "1 2").unwrap();
    assert_eq!(vector.vertical_bearing, -45);
    assert_eq!(vector.spatial, 2.0_f64.sqrt());
    assert!(!vector.text.contains("mark"));
    assert_eq!(
        battle_range_report(&scripts.world(), source, ObjectId(1), "1 2")
            .unwrap()
            .spatial,
        1.0
    );
    blind(&mut shared.borrow_mut(), map);
    assert!(battle_vector_report(&scripts.world(), source, ObjectId(1), "").is_err());
    assert!(battle_vector_report(&scripts.world(), source, ObjectId(1), "1 2 5").is_ok());
}

#[tokio::test]
async fn movement_queries_and_speed_names_share_native_lua_controls() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(source);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let before = scripts.world().btech.clone();
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "heading"),
        "Your current heading is 0."
    );
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(2), 1, "speed"),
        "Your current speed is 0.00."
    );
    let speed: f64 = scripts
        .eval_callback(&format!("return btech.unit.speed({},2)", source.0))
        .unwrap();
    let heading: f64 = scripts
        .eval_callback(&format!("return btech.unit.heading({},1)", source.0))
        .unwrap();
    assert_eq!((speed, heading), (0.0, 0.0));
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let maximum = scripts.world().btech.constructed_units()[&source].movement_maximum_speed();
    for (argument, expected) in [
        ("walk", maximum * 2.0 / 3.0),
        ("CRUISE", maximum * 2.0 / 3.0),
        ("run", maximum),
        ("flank", maximum),
        ("stop", 0.0),
        ("back", -maximum * 2.0 / 3.0),
        ("100000", maximum),
        ("-100000", -maximum * 2.0 / 3.0),
    ] {
        let _ = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("speed {argument}"),
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&source]
                .motion()
                .unwrap()
                .desired_speed,
            expected
        );
        let result: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.speed({},1,{:?})",
                source.0, argument
            ))
            .unwrap();
        assert!(result);
        assert_eq!(
            scripts.world().btech.constructed_units()[&source]
                .motion()
                .unwrap()
                .desired_speed,
            expected
        );
        assert_eq!(
            battle_motion_readout(&scripts.world(), source, ObjectId(1))
                .unwrap()
                .speed,
            0.0
        );
        let _ = scripts.drain_outbox();
    }
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.speed({},1,'flank'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    for value in ["true", "0/0", "math.huge", "'invalid'"] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.speed({},1,{value})", source.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.speed({},2,'stop')", source.0))
            .is_err()
    );
    let _: bool = scripts
        .eval_callback(&format!("return btech.unit.heading({},1,45)", source.0))
        .unwrap();
    let current: f64 = scripts
        .eval_callback(&format!("return btech.unit.heading({},1)", source.0))
        .unwrap();
    assert_eq!(current, 0.0);
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .motion()
            .unwrap()
            .desired_heading,
        45.0
    );
    let _ = scripts.drain_outbox();
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_motion_readout(&restored, source, ObjectId(1)).unwrap(),
        battle_motion_readout(&scripts.world(), source, ObjectId(1)).unwrap()
    );
}

#[tokio::test]
async fn stunned_speed_controls_obey_saved_recovery_without_blocking_walking() {
    let (_dir, config, mut world, _map, source, _target) = fixture().await;
    stun_battle_unit(&mut world, source).unwrap();
    for _ in 0..9 {
        advance_battle_stun(&mut world);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let world = persistence::load(&config.database()).await.unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared).unwrap();
    let maximum = scripts.world().btech.constructed_units()[&source].movement_maximum_speed();
    let walking = maximum * 2.0 / 3.0;
    for argument in ["run", "flank", "100000"] {
        let before = scripts.world().btech.clone();
        assert!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("speed {argument}")
            )
            .contains("while stunned")
        );
        assert_eq!(scripts.world().btech, before);
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.speed({},1,{argument:?})", source.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    for (argument, expected) in [("walk", walking), ("back", -walking), ("stop", 0.0)] {
        let _: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.speed({},1,{argument:?})",
                source.0
            ))
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&source]
                .motion()
                .unwrap()
                .desired_speed,
            expected
        );
        scripts.drain_outbox();
    }
    let before = scripts.world().btech.clone();
    assert!(
        set_battle_speed(
            &mut scripts.world_mut(),
            source,
            ObjectId(1),
            walking + 0.11
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    set_battle_speed(&mut scripts.world_mut(), source, ObjectId(1), walking + 0.1).unwrap();
    assert_eq!(advance_battle_stun(&mut scripts.world_mut()).len(), 1);
    let _: bool = scripts
        .eval_callback(&format!("return btech.unit.speed({},1,'run')", source.0))
        .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .motion()
            .unwrap()
            .desired_speed,
        maximum
    );
}

#[tokio::test]
async fn ammunition_dump_controls_timers_and_restart_share_native_lua_state() {
    let (_dir, config, mut world, _map, source, _) = fixture().await;
    let loadout = world.btech.constructed_units()[&source].loadout().unwrap();
    let weapon = loadout
        .weapons
        .iter()
        .position(|mount| mount.weapon == loadout.ammunition[0].weapon)
        .unwrap();
    let baseline = world.btech.clone();
    for argument in [
        "",
        "-1",
        "999",
        "RT 0",
        "RT 13",
        "RT nope",
        "all extra",
        "stop",
        "HD",
        "RT 2",
    ] {
        assert!(
            begin_battle_dump(&mut world, source, ObjectId(1), argument).is_err(),
            "{argument}"
        );
        assert_eq!(world.btech, baseline);
    }
    assert!(begin_battle_dump(&mut world, source, ObjectId(2), "all").is_err());
    let maximum = world.btech.constructed_units()[&source].movement_maximum_speed();
    set_battle_speed(&mut world, source, ObjectId(1), maximum).unwrap();
    assert!(begin_battle_dump(&mut world, source, ObjectId(1), "all").is_err());
    set_battle_speed(&mut world, source, ObjectId(1), 0.0).unwrap();
    let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
    let scripts = Scripts::new(&config, shared).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.dump({},1,'all'); error('abort')",
                source.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "dump RT 1")
            .contains("Starting dumping")
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&source].ammunition(),
        &[25]
    );
    let before = scripts.world().btech.clone();
    assert!(set_battle_speed(&mut scripts.world_mut(), source, ObjectId(1), maximum).is_err());
    assert!(launch_battle_jump(&mut scripts.world_mut(), source, ObjectId(1), 0, 1.0).is_err());
    assert!(begin_battle_dump(&mut scripts.world_mut(), source, ObjectId(1), "RT").is_err());
    assert_eq!(scripts.world().btech, before);
    let _: mlua::Value = scripts
        .eval_callback(&format!("return btech.unit.dump({},1,'all')", source.0))
        .unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&source]
            .dumping()
            .unwrap()
            .selection,
        BattleDumpSelection::All
    );
    for _ in 0..7 {
        advance_battle_dumping(&mut scripts.world_mut()).unwrap();
    }
    assert_eq!(
        scripts.world().btech.constructed_units()[&source].ammunition(),
        &[18]
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..17 {
        advance_battle_dumping(&mut scripts.world_mut()).unwrap();
        advance_battle_dumping(&mut restored).unwrap();
    }
    assert_eq!(
        advance_battle_dumping(&mut scripts.world_mut()).unwrap(),
        advance_battle_dumping(&mut restored).unwrap()
    );
    assert_eq!(scripts.world().btech, restored.btech);
    assert_eq!(
        restored.btech.constructed_units()[&source].ammunition(),
        &[0]
    );
    assert!(
        restored.btech.constructed_units()[&source]
            .dumping()
            .is_none()
    );
    set_battle_speed(&mut restored, source, ObjectId(1), maximum).unwrap();
    // A weapon selector and cancellation leave all remaining rounds in place.
    let mut world = saved;
    begin_battle_dump(&mut world, source, ObjectId(1), "stop").unwrap();
    begin_battle_dump(&mut world, source, ObjectId(1), &weapon.to_string()).unwrap();
    assert!(matches!(
        world.btech.constructed_units()[&source]
            .dumping()
            .unwrap()
            .selection,
        BattleDumpSelection::Weapon(_)
    ));
    begin_battle_dump(&mut world, source, ObjectId(1), "stop").unwrap();
    assert!(advance_battle_dumping(&mut world).unwrap().is_empty());
    assert_eq!(world.btech.constructed_units()[&source].ammunition(), &[18]);
}

#[tokio::test]
async fn ammunition_dump_low_capacity_bins_preserve_cadence_and_shutdown_cancels() {
    let (_dir, config, mut world, map, source, _) = fixture().await;
    stop_battle_unit(
        &mut world,
        source,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let id = world.create(&config, "Dumping Atlas".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 1, 3).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    let (index, bin) = loadout
        .ammunition
        .iter()
        .enumerate()
        .find(|(_, bin)| bin.weapon.profile().ammunition_per_ton == 5)
        .unwrap();
    let initial = world.btech.constructed_units()[&id].ammunition().to_vec();
    let argument = format!("{} {}", bin.location.section.name(), bin.location.slot + 1);
    begin_battle_dump(&mut world, id, ObjectId(1), &argument).unwrap();
    for _ in 0..5 {
        advance_battle_dumping(&mut world).unwrap();
    }
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), initial);
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    for second in 6..=30 {
        advance_battle_dumping(&mut world).unwrap();
        for (other, rounds) in world.btech.constructed_units()[&id]
            .ammunition()
            .iter()
            .enumerate()
        {
            assert_eq!(
                *rounds,
                initial[other] - if other == index { second / 6 } else { 0 }
            );
        }
    }
    assert!(world.btech.constructed_units()[&id].dumping().is_none());
    begin_battle_dump(&mut world, id, ObjectId(1), "all").unwrap();
    let before = world.btech.constructed_units()[&id].ammunition().to_vec();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(advance_battle_dumping(&mut world).unwrap().is_empty());
    assert!(world.btech.constructed_units()[&id].dumping().is_none());
    assert_eq!(world.btech.constructed_units()[&id].ammunition(), before);
}

#[tokio::test(flavor = "current_thread")]
async fn ammunition_dump_server_retries_failed_commits_without_losing_rounds() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, _, id, _) = fixture().await;
        begin_battle_dump(&mut world, id, ObjectId(1), "all").unwrap();
        for _ in 0..23 { advance_battle_dumping(&mut world).unwrap(); }
        assert_eq!(world.btech.constructed_units()[&id].ammunition(), &[2]);
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_dump BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'dump failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, before);
        sqlx::query("DROP TRIGGER deny_dump").execute(&mut sql).await.unwrap();
        let restored = heartbeats.until_saved(&config, 5, |restored| restored.btech.constructed_units()[&id].dumping().is_none()).await;
        assert_eq!(restored.btech.constructed_units()[&id].ammunition(), &[0]);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Each explicit radius is independent, falls back at zero, and halves with integer truncation.
#[test]
fn explicit_template_ranges_preserve_defaults_and_damage_limits() {
    for value in ["0", "1", "127"] {
        let mut template =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        for field in ["scan_range", "tac_range", "lrs_range"] {
            template.attributes.insert(field.into(), value.into());
        }
        template
            .attributes
            .insert("radio_range".into(), "32767".into());
        let mut unit = BattleUnit::from_template(template).unwrap();
        let expected = if value == "0" {
            BattleSensorRanges {
                tactical: 25,
                long_range: 50,
                scan: 25,
            }
        } else {
            let n = value.parse().unwrap();
            BattleSensorRanges {
                tactical: n,
                long_range: n,
                scan: n,
            }
        };
        assert_eq!(unit.sensor_ranges(), expected);
        assert_eq!(unit.radio_capabilities().range, 32767);
        unit.destroy_critical(CriticalLocation {
            section: BattleSection::Head,
            slot: 1,
        })
        .unwrap();
        assert_eq!(
            unit.sensor_ranges(),
            BattleSensorRanges {
                tactical: expected.tactical / 2,
                long_range: expected.long_range / 2,
                scan: expected.scan / 2
            }
        );
        unit.destroy_critical(CriticalLocation {
            section: BattleSection::Head,
            slot: 4,
        })
        .unwrap();
        assert_eq!(
            unit.sensor_ranges(),
            BattleSensorRanges {
                tactical: 0,
                long_range: 0,
                scan: 0
            }
        );
        assert_eq!(unit.radio_capabilities().range, 32767);
    }
    for field in ["scan_range", "tac_range", "lrs_range", "radio_range"] {
        for value in [
            "-1",
            "65536",
            "abc",
            "1.5",
            if field == "radio_range" {
                "32768"
            } else {
                "128"
            },
        ] {
            let mut template =
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap();
            template.attributes.insert(field.into(), value.into());
            assert!(
                BattleUnit::from_template(template).is_err(),
                "{field}={value}"
            );
        }
    }
}

/// Live scan, display and radio consumers share the saved template limits.
#[tokio::test]
async fn range_overrides_control_live_queries_delivery_and_restart() {
    let (_dir, config, mut world, map, source, target) = fixture_with_ranges(&[
        ("scan_range", "1"),
        ("tac_range", "3"),
        ("lrs_range", "7"),
        ("radio_range", "1"),
    ])
    .await;
    assert!(scan_battle_unit(&world, source, ObjectId(1), target, "I").is_ok());
    set_radio_mode(
        &mut world,
        source,
        ObjectId(1),
        0,
        BattleRadioMode {
            digital: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        resolve_digital_radio(&world, source, 0, "Within reach")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == target)
    );
    place_battle_unit(&mut world, target, map, 1, 3).unwrap();
    acquire(&mut world, source, target);
    assert!(
        scan_battle_unit(&world, source, ObjectId(1), target, "I")
            .unwrap_err()
            .to_string()
            .contains("range")
    );
    assert!(
        !resolve_digital_radio(&world, source, 0, "Beyond reach")
            .unwrap()
            .receptions
            .iter()
            .any(|r| r.receiver == target)
    );
    for (kind, limit) in [
        (BattleViewKind::Tactical, 3),
        (BattleViewKind::LongRange, 7),
    ] {
        assert_eq!(
            resolve_battle_view_center(
                &world,
                source,
                ObjectId(1),
                kind,
                BattleViewCenter::Projection {
                    bearing: 180,
                    distance: f64::from(limit)
                }
            )
            .unwrap()
            .maximum_range,
            limit
        );
        assert!(
            resolve_battle_view_center(
                &world,
                source,
                ObjectId(1),
                kind,
                BattleViewCenter::Projection {
                    bearing: 180,
                    distance: f64::from(limit + 1)
                }
            )
            .is_err()
        );
    }
    let before = world.btech.clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, before);
    assert_eq!(
        resolve_digital_radio(&restored, source, 0, "Beyond reach").unwrap(),
        resolve_digital_radio(&world, source, 0, "Beyond reach").unwrap()
    );
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let ranges: (u8,u8,u8,u16) = scripts.eval_callback(&format!("local s=assert(btech.unit.state({})); return s.sensor_ranges.tactical,s.sensor_ranges.long_range,s.sensor_ranges.scan,s.radio_capabilities.range",source.0)).unwrap();
    assert_eq!(ranges, (3, 7, 1, 1));
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}

/// The newest field determines whether a hex exposes a mine marker, independently of its slot.
#[tokio::test]
async fn tactical_mine_markers_follow_gameplay_insertion_order() {
    let (_dir, _config, mut world, map, source, _target) = fixture().await;
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 2 },
        kind: BattleMineKind::Standard,
        strength: 10,
        extra: 0,
        owner: ObjectId(1),
    };
    stompymux_rs::insert_minefield(&mut world, map, mine).unwrap();
    let render = |world: &World| {
        text::plain(
            &battle_tactical_map(
                world,
                source,
                ObjectId(1),
                "M",
                BattleViewDimensions::default(),
            )
            .unwrap()
            .text,
        )
    };
    assert_eq!(&render(&world).lines().nth(8).unwrap()[8..10], "<>");
    stompymux_rs::insert_minefield(
        &mut world,
        map,
        BattleMinefield {
            kind: BattleMineKind::Trigger,
            ..mine
        },
    )
    .unwrap();
    assert_eq!(&render(&world).lines().nth(8).unwrap()[8..10], "  ");
    stompymux_rs::insert_minefield(&mut world, map, mine).unwrap();
    assert_eq!(&render(&world).lines().nth(8).unwrap()[8..10], "<>");
}

/// Unknown single-letter flags use the reference rejection in native and Lua views.
#[tokio::test]
async fn tactical_unknown_flags_share_reference_rejection_without_mutation() {
    let (_dir, config, world, _map, source, _) = fixture().await;
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    for flag in ["q", "Q", "z", "Z"] {
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("tactical {flag}")
            ),
            "Invalid tactical map flag."
        );
        let error = scripts
            .eval_callback::<mlua::Table>(&format!(
                "return btech.unit.tactical({},1,'{flag}')",
                source.0
            ))
            .unwrap_err();
        assert!(error.to_string().contains("Invalid tactical map flag."));
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
}

/// Removed controls are absent from Lua and storage; bipeds cannot request lateral travel.
#[tokio::test]
async fn removed_movement_controls_and_biped_lateral_are_unavailable() {
    let (_dir, config, world, _map, source, _target) = fixture().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    scripts.eval_callback::<()>(&format!(
        "assert(btech.unit.turnmode == nil); local s=btech.unit.state({}); assert(s.tight_turn_mode == nil)",
        source.0
    )).unwrap();
    for direction in ["fl", "fr", "rl", "rr", "-"] {
        assert!(battle_lateral(&scripts, source, ObjectId(1), direction).is_err());
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.lateral({},1,'{direction}')", source.0))
                .is_err()
        );
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("lateral {direction}"),
        );
        assert!(
            output.contains("You cannot alter your lateral movement!"),
            "{output}"
        );
        assert_eq!(scripts.world().btech, before);
    }
    let state = serde_json::to_value(&before).unwrap();
    let unit = &state["constructed"][source.0.to_string()];
    assert!(unit.get("sprinting").is_none());
    assert!(unit.get("tight_turn_mode").is_none());
    let mut invalid = state;
    invalid["constructed"][source.0.to_string()]["lateral"] =
        serde_json::to_value(BattleLateralState {
            active: BattleLateralMode::FrontLeft,
            ..Default::default()
        })
        .unwrap();
    let mut world = scripts.world().clone();
    world.btech = serde_json::from_value(invalid).unwrap();
    assert!(world.validate(&config).is_err());
}
